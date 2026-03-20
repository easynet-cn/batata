package tests

import (
	"testing"
	"time"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// ==================== Service Defaults Config Entry Tests ====================

// TestConfigEntryServiceDefaults tests service-defaults config entry
func TestConfigEntryServiceDefaults(t *testing.T) {
	client := getTestClient(t)

	configEntries := client.ConfigEntries()
	serviceName := "service-defaults-" + randomString(8)

	entry := &api.ServiceConfigEntry{
		Kind:     api.ServiceDefaults,
		Name:     serviceName,
		Protocol: "http",
		MeshGateway: api.MeshGatewayConfig{
			Mode: api.MeshGatewayModeLocal,
		},
	}

	success, _, err := configEntries.Set(entry, nil)
	require.NoError(t, err, "Failed to create service defaults config entry")
	assert.True(t, success, "Set should return success")

	// Get the entry
	gotEntry, _, err := configEntries.Get(api.ServiceDefaults, serviceName, nil)
	require.NoError(t, err, "Failed to get service defaults config entry")
	require.NotNil(t, gotEntry, "Got entry should not be nil")

	serviceEntry := gotEntry.(*api.ServiceConfigEntry)
	assert.Equal(t, api.ServiceDefaults, serviceEntry.Kind, "Kind should be service-defaults")
	assert.Equal(t, serviceName, serviceEntry.Name, "Name should match")
	assert.Equal(t, "http", serviceEntry.Protocol, "Protocol should be http")
	assert.Equal(t, api.MeshGatewayModeLocal, serviceEntry.MeshGateway.Mode, "MeshGateway mode should be local")

	// Cleanup
	_, err = configEntries.Delete(api.ServiceDefaults, serviceName, nil)
	assert.NoError(t, err, "Failed to delete service defaults config entry")
}

// TestConfigEntryServiceDefaultsWithUpstream tests service defaults with upstream config
func TestConfigEntryServiceDefaultsWithUpstream(t *testing.T) {
	client := getTestClient(t)

	configEntries := client.ConfigEntries()
	serviceName := "upstream-defaults-" + randomString(8)

	entry := &api.ServiceConfigEntry{
		Kind:     api.ServiceDefaults,
		Name:     serviceName,
		Protocol: "grpc",
		UpstreamConfig: &api.UpstreamConfiguration{
			Defaults: &api.UpstreamConfig{
				ConnectTimeoutMs: 5000,
				MeshGateway: api.MeshGatewayConfig{
					Mode: api.MeshGatewayModeRemote,
				},
			},
		},
	}

	success, _, err := configEntries.Set(entry, nil)
	require.NoError(t, err, "Failed to create service defaults with upstream config")
	assert.True(t, success, "Set should return success")

	// Get the entry and verify upstream config
	gotEntry, _, err := configEntries.Get(api.ServiceDefaults, serviceName, nil)
	require.NoError(t, err, "Failed to get service defaults with upstream config")
	require.NotNil(t, gotEntry, "Got entry should not be nil")

	serviceEntry := gotEntry.(*api.ServiceConfigEntry)
	assert.Equal(t, "grpc", serviceEntry.Protocol, "Protocol should be grpc")
	require.NotNil(t, serviceEntry.UpstreamConfig, "UpstreamConfig should not be nil")
	require.NotNil(t, serviceEntry.UpstreamConfig.Defaults, "UpstreamConfig.Defaults should not be nil")
	assert.Equal(t, 5000, serviceEntry.UpstreamConfig.Defaults.ConnectTimeoutMs, "ConnectTimeoutMs should be 5000")
	assert.Equal(t, api.MeshGatewayModeRemote, serviceEntry.UpstreamConfig.Defaults.MeshGateway.Mode, "Upstream MeshGateway mode should be remote")

	// Cleanup
	_, err = configEntries.Delete(api.ServiceDefaults, serviceName, nil)
	assert.NoError(t, err, "Failed to delete service defaults config entry")
}

// ==================== Proxy Defaults Config Entry Tests ====================

// TestConfigEntryProxyDefaults tests proxy-defaults config entry
func TestConfigEntryProxyDefaults(t *testing.T) {
	client := getTestClient(t)

	configEntries := client.ConfigEntries()

	entry := &api.ProxyConfigEntry{
		Kind: api.ProxyDefaults,
		Name: api.ProxyConfigGlobal,
		Config: map[string]interface{}{
			"protocol": "http",
		},
		MeshGateway: api.MeshGatewayConfig{
			Mode: api.MeshGatewayModeLocal,
		},
	}

	success, _, err := configEntries.Set(entry, nil)
	require.NoError(t, err, "Failed to create proxy defaults config entry")
	assert.True(t, success, "Set should return success")

	// Get the entry
	gotEntry, _, err := configEntries.Get(api.ProxyDefaults, api.ProxyConfigGlobal, nil)
	require.NoError(t, err, "Failed to get proxy defaults config entry")
	require.NotNil(t, gotEntry, "Got entry should not be nil")

	proxyEntry := gotEntry.(*api.ProxyConfigEntry)
	assert.Equal(t, api.ProxyDefaults, proxyEntry.Kind, "Kind should be proxy-defaults")
	assert.Equal(t, api.ProxyConfigGlobal, proxyEntry.Name, "Name should be global")
	require.NotNil(t, proxyEntry.Config, "Config should not be nil")
	assert.Equal(t, "http", proxyEntry.Config["protocol"], "Config protocol should be http")
	assert.Equal(t, api.MeshGatewayModeLocal, proxyEntry.MeshGateway.Mode, "MeshGateway mode should be local")

	// Cleanup
	_, err = configEntries.Delete(api.ProxyDefaults, api.ProxyConfigGlobal, nil)
	assert.NoError(t, err, "Failed to delete proxy defaults config entry")
}

// ==================== Service Router Config Entry Tests ====================

// TestConfigEntryServiceRouter tests service-router config entry
func TestConfigEntryServiceRouter(t *testing.T) {
	client := getTestClient(t)

	configEntries := client.ConfigEntries()
	serviceName := "router-service-" + randomString(8)

	// First create service defaults (required for router)
	defaults := &api.ServiceConfigEntry{
		Kind:     api.ServiceDefaults,
		Name:     serviceName,
		Protocol: "http",
	}
	_, _, err := configEntries.Set(defaults, nil)
	require.NoError(t, err, "Failed to create service defaults prerequisite for router")

	// Create router
	router := &api.ServiceRouterConfigEntry{
		Kind: api.ServiceRouter,
		Name: serviceName,
		Routes: []api.ServiceRoute{
			{
				Match: &api.ServiceRouteMatch{
					HTTP: &api.ServiceRouteHTTPMatch{
						PathPrefix: "/api/v1",
					},
				},
				Destination: &api.ServiceRouteDestination{
					Service:       serviceName,
					ServiceSubset: "v1",
				},
			},
			{
				Match: &api.ServiceRouteMatch{
					HTTP: &api.ServiceRouteHTTPMatch{
						PathPrefix: "/api/v2",
					},
				},
				Destination: &api.ServiceRouteDestination{
					Service:       serviceName,
					ServiceSubset: "v2",
				},
			},
		},
	}

	success, _, err := configEntries.Set(router, nil)
	require.NoError(t, err, "Failed to create service router config entry")
	assert.True(t, success, "Set should return success")

	// Get the entry and verify routes
	gotEntry, _, err := configEntries.Get(api.ServiceRouter, serviceName, nil)
	require.NoError(t, err, "Failed to get service router config entry")
	require.NotNil(t, gotEntry, "Got entry should not be nil")

	routerEntry := gotEntry.(*api.ServiceRouterConfigEntry)
	assert.Equal(t, api.ServiceRouter, routerEntry.Kind, "Kind should be service-router")
	assert.Equal(t, serviceName, routerEntry.Name, "Name should match")
	require.Len(t, routerEntry.Routes, 2, "Should have 2 routes")

	// Verify first route
	require.NotNil(t, routerEntry.Routes[0].Match, "First route match should not be nil")
	require.NotNil(t, routerEntry.Routes[0].Match.HTTP, "First route HTTP match should not be nil")
	assert.Equal(t, "/api/v1", routerEntry.Routes[0].Match.HTTP.PathPrefix, "First route path prefix should be /api/v1")
	require.NotNil(t, routerEntry.Routes[0].Destination, "First route destination should not be nil")
	assert.Equal(t, "v1", routerEntry.Routes[0].Destination.ServiceSubset, "First route subset should be v1")

	// Verify second route
	require.NotNil(t, routerEntry.Routes[1].Match, "Second route match should not be nil")
	require.NotNil(t, routerEntry.Routes[1].Match.HTTP, "Second route HTTP match should not be nil")
	assert.Equal(t, "/api/v2", routerEntry.Routes[1].Match.HTTP.PathPrefix, "Second route path prefix should be /api/v2")
	require.NotNil(t, routerEntry.Routes[1].Destination, "Second route destination should not be nil")
	assert.Equal(t, "v2", routerEntry.Routes[1].Destination.ServiceSubset, "Second route subset should be v2")

	// Cleanup
	configEntries.Delete(api.ServiceRouter, serviceName, nil)
	_, err = configEntries.Delete(api.ServiceDefaults, serviceName, nil)
	assert.NoError(t, err, "Failed to delete service defaults config entry")
}

// ==================== Service Splitter Config Entry Tests ====================

// TestConfigEntryServiceSplitter tests service-splitter config entry
func TestConfigEntryServiceSplitter(t *testing.T) {
	client := getTestClient(t)

	configEntries := client.ConfigEntries()
	serviceName := "splitter-service-" + randomString(8)

	// First create service defaults
	defaults := &api.ServiceConfigEntry{
		Kind:     api.ServiceDefaults,
		Name:     serviceName,
		Protocol: "http",
	}
	_, _, err := configEntries.Set(defaults, nil)
	require.NoError(t, err, "Failed to create service defaults prerequisite for splitter")

	// Create splitter for canary deployment
	splitter := &api.ServiceSplitterConfigEntry{
		Kind: api.ServiceSplitter,
		Name: serviceName,
		Splits: []api.ServiceSplit{
			{
				Weight:        90,
				ServiceSubset: "stable",
			},
			{
				Weight:        10,
				ServiceSubset: "canary",
			},
		},
	}

	success, _, err := configEntries.Set(splitter, nil)
	require.NoError(t, err, "Failed to create service splitter config entry")
	assert.True(t, success, "Set should return success")

	// Get the entry and verify splits
	gotEntry, _, err := configEntries.Get(api.ServiceSplitter, serviceName, nil)
	require.NoError(t, err, "Failed to get service splitter config entry")
	require.NotNil(t, gotEntry, "Got entry should not be nil")

	splitterEntry := gotEntry.(*api.ServiceSplitterConfigEntry)
	assert.Equal(t, api.ServiceSplitter, splitterEntry.Kind, "Kind should be service-splitter")
	assert.Equal(t, serviceName, splitterEntry.Name, "Name should match")
	require.Len(t, splitterEntry.Splits, 2, "Should have 2 splits")
	assert.Equal(t, float32(90), splitterEntry.Splits[0].Weight, "First split weight should be 90")
	assert.Equal(t, "stable", splitterEntry.Splits[0].ServiceSubset, "First split subset should be stable")
	assert.Equal(t, float32(10), splitterEntry.Splits[1].Weight, "Second split weight should be 10")
	assert.Equal(t, "canary", splitterEntry.Splits[1].ServiceSubset, "Second split subset should be canary")

	// Cleanup
	configEntries.Delete(api.ServiceSplitter, serviceName, nil)
	_, err = configEntries.Delete(api.ServiceDefaults, serviceName, nil)
	assert.NoError(t, err, "Failed to delete service defaults config entry")
}

// ==================== Service Resolver Config Entry Tests ====================

// TestConfigEntryServiceResolver tests service-resolver config entry
func TestConfigEntryServiceResolver(t *testing.T) {
	client := getTestClient(t)

	configEntries := client.ConfigEntries()
	serviceName := "resolver-service-" + randomString(8)

	resolver := &api.ServiceResolverConfigEntry{
		Kind:          api.ServiceResolver,
		Name:          serviceName,
		DefaultSubset: "stable",
		Subsets: map[string]api.ServiceResolverSubset{
			"stable": {
				Filter: "Service.Meta.version == v1",
			},
			"canary": {
				Filter: "Service.Meta.version == v2",
			},
		},
		ConnectTimeout: 10 * time.Second,
	}

	success, _, err := configEntries.Set(resolver, nil)
	require.NoError(t, err, "Failed to create service resolver config entry")
	assert.True(t, success, "Set should return success")

	// Get the entry and verify subsets
	gotEntry, _, err := configEntries.Get(api.ServiceResolver, serviceName, nil)
	require.NoError(t, err, "Failed to get service resolver config entry")
	require.NotNil(t, gotEntry, "Got entry should not be nil")

	resolverEntry := gotEntry.(*api.ServiceResolverConfigEntry)
	assert.Equal(t, api.ServiceResolver, resolverEntry.Kind, "Kind should be service-resolver")
	assert.Equal(t, serviceName, resolverEntry.Name, "Name should match")
	assert.Equal(t, "stable", resolverEntry.DefaultSubset, "DefaultSubset should be stable")
	assert.Equal(t, 10*time.Second, resolverEntry.ConnectTimeout, "ConnectTimeout should be 10s")
	require.Len(t, resolverEntry.Subsets, 2, "Should have 2 subsets")

	stableSubset, ok := resolverEntry.Subsets["stable"]
	require.True(t, ok, "Should have stable subset")
	assert.Equal(t, "Service.Meta.version == v1", stableSubset.Filter, "Stable subset filter should match")

	canarySubset, ok := resolverEntry.Subsets["canary"]
	require.True(t, ok, "Should have canary subset")
	assert.Equal(t, "Service.Meta.version == v2", canarySubset.Filter, "Canary subset filter should match")

	// Cleanup
	_, err = configEntries.Delete(api.ServiceResolver, serviceName, nil)
	assert.NoError(t, err, "Failed to delete service resolver config entry")
}

// TestConfigEntryServiceResolverWithFailover tests service resolver with failover
func TestConfigEntryServiceResolverWithFailover(t *testing.T) {
	client := getTestClient(t)

	configEntries := client.ConfigEntries()
	serviceName := "failover-resolver-" + randomString(8)

	resolver := &api.ServiceResolverConfigEntry{
		Kind: api.ServiceResolver,
		Name: serviceName,
		Failover: map[string]api.ServiceResolverFailover{
			"*": {
				Datacenters: []string{"dc2", "dc3"},
			},
		},
	}

	success, _, err := configEntries.Set(resolver, nil)
	require.NoError(t, err, "Failed to create service resolver with failover")
	assert.True(t, success, "Set should return success")

	// Get the entry and verify failover
	gotEntry, _, err := configEntries.Get(api.ServiceResolver, serviceName, nil)
	require.NoError(t, err, "Failed to get service resolver with failover")
	require.NotNil(t, gotEntry, "Got entry should not be nil")

	resolverEntry := gotEntry.(*api.ServiceResolverConfigEntry)
	require.NotNil(t, resolverEntry.Failover, "Failover should not be nil")
	require.Contains(t, resolverEntry.Failover, "*", "Failover should contain wildcard key")
	assert.Equal(t, []string{"dc2", "dc3"}, resolverEntry.Failover["*"].Datacenters, "Failover datacenters should match")

	// Cleanup
	_, err = configEntries.Delete(api.ServiceResolver, serviceName, nil)
	assert.NoError(t, err, "Failed to delete service resolver config entry")
}

// ==================== Ingress Gateway Config Entry Tests ====================

// TestConfigEntryIngressGateway tests ingress-gateway config entry
func TestConfigEntryIngressGateway(t *testing.T) {
	client := getTestClient(t)

	configEntries := client.ConfigEntries()
	gatewayName := "ingress-gateway-" + randomString(8)

	ingress := &api.IngressGatewayConfigEntry{
		Kind: api.IngressGateway,
		Name: gatewayName,
		Listeners: []api.IngressListener{
			{
				Port:     8080,
				Protocol: "http",
				Services: []api.IngressService{
					{
						Name: "*",
					},
				},
			},
		},
	}

	success, _, err := configEntries.Set(ingress, nil)
	require.NoError(t, err, "Failed to create ingress gateway config entry")
	assert.True(t, success, "Set should return success")

	// Get the entry and verify listeners
	gotEntry, _, err := configEntries.Get(api.IngressGateway, gatewayName, nil)
	require.NoError(t, err, "Failed to get ingress gateway config entry")
	require.NotNil(t, gotEntry, "Got entry should not be nil")

	ingressEntry := gotEntry.(*api.IngressGatewayConfigEntry)
	assert.Equal(t, api.IngressGateway, ingressEntry.Kind, "Kind should be ingress-gateway")
	assert.Equal(t, gatewayName, ingressEntry.Name, "Name should match")
	require.Len(t, ingressEntry.Listeners, 1, "Should have 1 listener")
	assert.Equal(t, 8080, ingressEntry.Listeners[0].Port, "Listener port should be 8080")
	assert.Equal(t, "http", ingressEntry.Listeners[0].Protocol, "Listener protocol should be http")
	require.Len(t, ingressEntry.Listeners[0].Services, 1, "Listener should have 1 service")
	assert.Equal(t, "*", ingressEntry.Listeners[0].Services[0].Name, "Listener service name should be wildcard")

	// Cleanup
	_, err = configEntries.Delete(api.IngressGateway, gatewayName, nil)
	assert.NoError(t, err, "Failed to delete ingress gateway config entry")
}

// ==================== Terminating Gateway Config Entry Tests ====================

// TestConfigEntryTerminatingGateway tests terminating-gateway config entry
func TestConfigEntryTerminatingGateway(t *testing.T) {
	client := getTestClient(t)

	configEntries := client.ConfigEntries()
	gatewayName := "terminating-gateway-" + randomString(8)

	terminating := &api.TerminatingGatewayConfigEntry{
		Kind: api.TerminatingGateway,
		Name: gatewayName,
		Services: []api.LinkedService{
			{
				Name: "external-db",
			},
			{
				Name:     "external-api",
				CAFile:   "/etc/ssl/certs/ca.pem",
				CertFile: "/etc/ssl/certs/client.pem",
				KeyFile:  "/etc/ssl/certs/client-key.pem",
			},
		},
	}

	success, _, err := configEntries.Set(terminating, nil)
	require.NoError(t, err, "Failed to create terminating gateway config entry")
	assert.True(t, success, "Set should return success")

	// Get the entry and verify services
	gotEntry, _, err := configEntries.Get(api.TerminatingGateway, gatewayName, nil)
	require.NoError(t, err, "Failed to get terminating gateway config entry")
	require.NotNil(t, gotEntry, "Got entry should not be nil")

	terminatingEntry := gotEntry.(*api.TerminatingGatewayConfigEntry)
	assert.Equal(t, api.TerminatingGateway, terminatingEntry.Kind, "Kind should be terminating-gateway")
	assert.Equal(t, gatewayName, terminatingEntry.Name, "Name should match")
	require.Len(t, terminatingEntry.Services, 2, "Should have 2 linked services")
	assert.Equal(t, "external-db", terminatingEntry.Services[0].Name, "First service name should be external-db")
	assert.Equal(t, "external-api", terminatingEntry.Services[1].Name, "Second service name should be external-api")
	assert.Equal(t, "/etc/ssl/certs/ca.pem", terminatingEntry.Services[1].CAFile, "Second service CAFile should match")
	assert.Equal(t, "/etc/ssl/certs/client.pem", terminatingEntry.Services[1].CertFile, "Second service CertFile should match")
	assert.Equal(t, "/etc/ssl/certs/client-key.pem", terminatingEntry.Services[1].KeyFile, "Second service KeyFile should match")

	// Cleanup
	_, err = configEntries.Delete(api.TerminatingGateway, gatewayName, nil)
	assert.NoError(t, err, "Failed to delete terminating gateway config entry")
}

// ==================== Config Entry List Tests ====================

// TestConfigEntryList tests listing config entries
func TestConfigEntryList(t *testing.T) {
	client := getTestClient(t)

	configEntries := client.ConfigEntries()

	// Create a known entry first so the list is non-empty
	serviceName := "list-test-" + randomString(8)
	entry := &api.ServiceConfigEntry{
		Kind:     api.ServiceDefaults,
		Name:     serviceName,
		Protocol: "http",
	}
	_, _, err := configEntries.Set(entry, nil)
	require.NoError(t, err, "Failed to create service defaults for list test")

	// List all service-defaults entries
	entries, _, err := configEntries.List(api.ServiceDefaults, nil)
	require.NoError(t, err, "Failed to list service-defaults entries")
	assert.GreaterOrEqual(t, len(entries), 1, "Should have at least 1 service-defaults entry")

	// Verify our entry is in the list
	found := false
	for _, e := range entries {
		if e.GetName() == serviceName {
			found = true
			break
		}
	}
	assert.True(t, found, "Created entry should appear in the list")

	// Cleanup
	_, err = configEntries.Delete(api.ServiceDefaults, serviceName, nil)
	assert.NoError(t, err, "Failed to delete service defaults config entry")
}

// TestConfigEntryListByKind tests listing config entries by different kinds
func TestConfigEntryListByKind(t *testing.T) {
	client := getTestClient(t)

	configEntries := client.ConfigEntries()

	// Create one entry of a known kind so we can assert at least one result
	serviceName := "list-by-kind-" + randomString(8)
	entry := &api.ServiceConfigEntry{
		Kind:     api.ServiceDefaults,
		Name:     serviceName,
		Protocol: "http",
	}
	_, _, err := configEntries.Set(entry, nil)
	require.NoError(t, err, "Failed to create service defaults for list-by-kind test")

	kinds := []string{
		api.ServiceDefaults,
		api.ProxyDefaults,
		api.ServiceRouter,
		api.ServiceSplitter,
		api.ServiceResolver,
		api.IngressGateway,
		api.TerminatingGateway,
	}

	for _, kind := range kinds {
		entries, _, err := configEntries.List(kind, nil)
		require.NoError(t, err, "Listing %s should not return an error", kind)

		// Service defaults should have at least our entry
		if kind == api.ServiceDefaults {
			require.NotNil(t, entries, "service-defaults list should not be nil")
			assert.GreaterOrEqual(t, len(entries), 1, "service-defaults should have at least 1 entry")
		}
		// Other kinds may return nil (Go SDK returns nil for empty JSON array [])
	}

	// Cleanup
	_, err = configEntries.Delete(api.ServiceDefaults, serviceName, nil)
	assert.NoError(t, err, "Failed to delete service defaults config entry")
}

// ==================== Config Entry CAS Tests ====================

// TestConfigEntryCAS tests compare-and-set for config entries
func TestConfigEntryCAS(t *testing.T) {
	client := getTestClient(t)

	configEntries := client.ConfigEntries()
	serviceName := "cas-service-" + randomString(8)

	// Create initial entry
	entry := &api.ServiceConfigEntry{
		Kind:     api.ServiceDefaults,
		Name:     serviceName,
		Protocol: "http",
	}

	success, _, err := configEntries.Set(entry, nil)
	require.NoError(t, err, "Failed to create initial config entry for CAS test")
	assert.True(t, success, "Initial Set should return success")

	// Get the entry to get its modify index
	gotEntry, qm, err := configEntries.Get(api.ServiceDefaults, serviceName, nil)
	require.NoError(t, err, "Failed to get entry for CAS test")
	require.NotNil(t, gotEntry, "Got entry should not be nil")
	assert.True(t, qm.LastIndex > 0, "LastIndex should be greater than 0")

	// Update with CAS using the correct modify index
	serviceEntry := gotEntry.(*api.ServiceConfigEntry)
	serviceEntry.Protocol = "grpc"

	success, _, err = configEntries.CAS(serviceEntry, qm.LastIndex, nil)
	require.NoError(t, err, "CAS update with correct index should not return an error")
	assert.True(t, success, "CAS update with correct modify index should succeed")

	// Verify the update took effect
	updatedEntry, _, err := configEntries.Get(api.ServiceDefaults, serviceName, nil)
	require.NoError(t, err, "Failed to get updated entry")
	updatedServiceEntry := updatedEntry.(*api.ServiceConfigEntry)
	assert.Equal(t, "grpc", updatedServiceEntry.Protocol, "Protocol should be updated to grpc after CAS")

	// Try CAS with a stale (old) modify index - should fail
	updatedServiceEntry.Protocol = "tcp"
	success, _, err = configEntries.CAS(updatedServiceEntry, qm.LastIndex, nil)
	if err == nil {
		assert.False(t, success, "CAS update with stale modify index should fail")
	}

	// Cleanup
	_, err = configEntries.Delete(api.ServiceDefaults, serviceName, nil)
	assert.NoError(t, err, "Failed to delete config entry")
}

// ==================== Exported Services Config Entry Tests ====================

// TestConfigEntryExportedServices tests exported-services config entry
func TestConfigEntryExportedServices(t *testing.T) {
	client := getTestClient(t)

	configEntries := client.ConfigEntries()

	exported := &api.ExportedServicesConfigEntry{
		Name: "default",
		Services: []api.ExportedService{
			{
				Name: "web",
				Consumers: []api.ServiceConsumer{
					{
						Peer: "peer-dc2",
					},
				},
			},
		},
	}

	success, _, err := configEntries.Set(exported, nil)
	require.NoError(t, err, "Failed to create exported services config entry")
	assert.True(t, success, "Set should return success")

	// Get the entry and verify
	gotEntry, _, err := configEntries.Get(api.ExportedServices, "default", nil)
	require.NoError(t, err, "Failed to get exported services config entry")
	require.NotNil(t, gotEntry, "Got entry should not be nil")

	exportedEntry := gotEntry.(*api.ExportedServicesConfigEntry)
	assert.Equal(t, "default", exportedEntry.Name, "Name should be default")
	require.Len(t, exportedEntry.Services, 1, "Should have 1 exported service")
	assert.Equal(t, "web", exportedEntry.Services[0].Name, "Exported service name should be web")
	require.Len(t, exportedEntry.Services[0].Consumers, 1, "Should have 1 consumer")
	assert.Equal(t, "peer-dc2", exportedEntry.Services[0].Consumers[0].Peer, "Consumer peer should be peer-dc2")

	// Cleanup
	_, err = configEntries.Delete(api.ExportedServices, "default", nil)
	assert.NoError(t, err, "Failed to delete exported services config entry")
}

// ==================== Mesh Config Entry Tests ====================

// TestConfigEntryMesh tests mesh config entry
func TestConfigEntryMesh(t *testing.T) {
	client := getTestClient(t)

	configEntries := client.ConfigEntries()

	mesh := &api.MeshConfigEntry{
		TransparentProxy: api.TransparentProxyMeshConfig{
			MeshDestinationsOnly: true,
		},
	}

	success, _, err := configEntries.Set(mesh, nil)
	require.NoError(t, err, "Failed to create mesh config entry")
	assert.True(t, success, "Set should return success")

	// Get the entry and verify
	gotEntry, _, err := configEntries.Get(api.MeshConfig, api.MeshConfigMesh, nil)
	require.NoError(t, err, "Failed to get mesh config entry")
	require.NotNil(t, gotEntry, "Got entry should not be nil")

	meshEntry := gotEntry.(*api.MeshConfigEntry)
	assert.True(t, meshEntry.TransparentProxy.MeshDestinationsOnly, "MeshDestinationsOnly should be true")

	// Cleanup
	_, err = configEntries.Delete(api.MeshConfig, api.MeshConfigMesh, nil)
	assert.NoError(t, err, "Failed to delete mesh config entry")
}
