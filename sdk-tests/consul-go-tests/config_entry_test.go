package consultest

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

	_, _, err := configEntries.Set(entry, nil)
	if err != nil {
		t.Logf("Service defaults config entry not available: %v", err)
		return
	}

	// Get the entry
	gotEntry, _, err := configEntries.Get(api.ServiceDefaults, serviceName, nil)
	require.NoError(t, err)

	serviceEntry := gotEntry.(*api.ServiceConfigEntry)
	assert.Equal(t, "http", serviceEntry.Protocol)
	t.Logf("Service defaults created: %s with protocol %s", serviceName, serviceEntry.Protocol)

	// Cleanup
	configEntries.Delete(api.ServiceDefaults, serviceName, nil)
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

	_, _, err := configEntries.Set(entry, nil)
	if err != nil {
		t.Logf("Service defaults with upstream not available: %v", err)
		return
	}

	t.Logf("Service defaults with upstream config created: %s", serviceName)

	// Cleanup
	configEntries.Delete(api.ServiceDefaults, serviceName, nil)
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

	_, _, err := configEntries.Set(entry, nil)
	if err != nil {
		t.Logf("Proxy defaults config entry not available: %v", err)
		return
	}

	// Get the entry
	gotEntry, _, err := configEntries.Get(api.ProxyDefaults, api.ProxyConfigGlobal, nil)
	if err != nil {
		t.Logf("Get proxy defaults: %v", err)
		return
	}

	proxyEntry := gotEntry.(*api.ProxyConfigEntry)
	t.Logf("Proxy defaults config: %v", proxyEntry.Config)

	// Cleanup
	configEntries.Delete(api.ProxyDefaults, api.ProxyConfigGlobal, nil)
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
	configEntries.Set(defaults, nil)

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

	_, _, err := configEntries.Set(router, nil)
	if err != nil {
		t.Logf("Service router config entry not available: %v", err)
		// Cleanup
		configEntries.Delete(api.ServiceDefaults, serviceName, nil)
		return
	}

	t.Logf("Service router created with %d routes", len(router.Routes))

	// Cleanup
	configEntries.Delete(api.ServiceRouter, serviceName, nil)
	configEntries.Delete(api.ServiceDefaults, serviceName, nil)
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
	configEntries.Set(defaults, nil)

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

	_, _, err := configEntries.Set(splitter, nil)
	if err != nil {
		t.Logf("Service splitter config entry not available: %v", err)
		configEntries.Delete(api.ServiceDefaults, serviceName, nil)
		return
	}

	t.Logf("Service splitter created: 90%% stable, 10%% canary")

	// Cleanup
	configEntries.Delete(api.ServiceSplitter, serviceName, nil)
	configEntries.Delete(api.ServiceDefaults, serviceName, nil)
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

	_, _, err := configEntries.Set(resolver, nil)
	if err != nil {
		t.Logf("Service resolver config entry not available: %v", err)
		return
	}

	t.Logf("Service resolver created with subsets: stable, canary")

	// Cleanup
	configEntries.Delete(api.ServiceResolver, serviceName, nil)
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

	_, _, err := configEntries.Set(resolver, nil)
	if err != nil {
		t.Logf("Service resolver with failover not available: %v", err)
		return
	}

	t.Logf("Service resolver with failover to dc2, dc3 created")

	// Cleanup
	configEntries.Delete(api.ServiceResolver, serviceName, nil)
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

	_, _, err := configEntries.Set(ingress, nil)
	if err != nil {
		t.Logf("Ingress gateway config entry not available: %v", err)
		return
	}

	t.Logf("Ingress gateway config created: %s on port 8080", gatewayName)

	// Cleanup
	configEntries.Delete(api.IngressGateway, gatewayName, nil)
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

	_, _, err := configEntries.Set(terminating, nil)
	if err != nil {
		t.Logf("Terminating gateway config entry not available: %v", err)
		return
	}

	t.Logf("Terminating gateway config created: %s", gatewayName)

	// Cleanup
	configEntries.Delete(api.TerminatingGateway, gatewayName, nil)
}

// ==================== Config Entry List Tests ====================

// TestConfigEntryList tests listing config entries
func TestConfigEntryList(t *testing.T) {
	client := getTestClient(t)

	configEntries := client.ConfigEntries()

	// List all service-defaults entries
	entries, _, err := configEntries.List(api.ServiceDefaults, nil)
	if err != nil {
		t.Logf("Config entry list not available: %v", err)
		return
	}

	t.Logf("Found %d service-defaults entries", len(entries))
	for _, entry := range entries {
		t.Logf("  - %s", entry.GetName())
	}
}

// TestConfigEntryListByKind tests listing config entries by different kinds
func TestConfigEntryListByKind(t *testing.T) {
	client := getTestClient(t)

	configEntries := client.ConfigEntries()

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
		if err != nil {
			t.Logf("List %s: %v", kind, err)
			continue
		}
		t.Logf("%s entries: %d", kind, len(entries))
	}
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
	if err != nil {
		t.Logf("Config entry CAS not available: %v", err)
		return
	}

	t.Logf("Initial entry created, success: %v", success)

	// Get the entry to get its modify index
	gotEntry, qm, err := configEntries.Get(api.ServiceDefaults, serviceName, nil)
	if err != nil {
		t.Logf("Get entry for CAS: %v", err)
		configEntries.Delete(api.ServiceDefaults, serviceName, nil)
		return
	}

	t.Logf("Entry modify index: %d", qm.LastIndex)

	// Update with CAS
	serviceEntry := gotEntry.(*api.ServiceConfigEntry)
	serviceEntry.Protocol = "grpc"

	success, _, err = configEntries.CAS(serviceEntry, qm.LastIndex, nil)
	if err != nil {
		t.Logf("CAS update: %v", err)
	} else {
		t.Logf("CAS update success: %v", success)
	}

	// Cleanup
	configEntries.Delete(api.ServiceDefaults, serviceName, nil)
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

	_, _, err := configEntries.Set(exported, nil)
	if err != nil {
		t.Logf("Exported services config entry not available: %v", err)
		return
	}

	t.Logf("Exported services config created")

	// Cleanup
	configEntries.Delete(api.ExportedServices, "default", nil)
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

	_, _, err := configEntries.Set(mesh, nil)
	if err != nil {
		t.Logf("Mesh config entry not available: %v", err)
		return
	}

	t.Logf("Mesh config entry created")

	// Cleanup
	configEntries.Delete(api.MeshConfig, api.MeshConfigMesh, nil)
}
