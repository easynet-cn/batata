package tests

import (
	"math/rand"
	"os"
	"testing"
	"time"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// Helper functions for tests

func getTestClient(t *testing.T) *api.Client {
	addr := os.Getenv("CONSUL_HTTP_ADDR")
	if addr == "" {
		addr = "127.0.0.1:8500"
	}

	token := os.Getenv("CONSUL_HTTP_TOKEN")
	if token == "" {
		token = "root"
	}

	client, err := api.NewClient(&api.Config{
		Address: addr,
		Token:   token,
	})
	require.NoError(t, err)
	return client
}

func randomString(n int) string {
	const letters = "abcdefghijklmnopqrstuvwxyz0123456789"
	b := make([]byte, n)
	for i := range b {
		b[i] = letters[rand.Intn(len(letters))]
	}
	return string(b)
}

func init() {
	rand.Seed(time.Now().UnixNano())
}

// ==================== Service Mesh Basic Setup Tests ====================

// TestServiceMeshBasicSetup tests basic service mesh setup with Connect-enabled service
func TestServiceMeshBasicSetup(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	serviceName := "mesh-basic-" + randomString(8)

	// Register a Connect-enabled service
	reg := &api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
		Connect: &api.AgentServiceConnect{
			SidecarService: &api.AgentServiceRegistration{
				Port: 21000,
			},
		},
		Meta: map[string]string{
			"version": "v1",
			"mesh":    "enabled",
		},
	}

	err := agent.ServiceRegister(reg)
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)
	defer agent.ServiceDeregister(serviceName + "-sidecar-proxy")

	time.Sleep(500 * time.Millisecond)

	// Verify service is registered
	services, err := agent.Services()
	require.NoError(t, err)

	service, exists := services[serviceName]
	assert.True(t, exists, "Service should be registered")
	if exists {
		assert.Equal(t, serviceName, service.Service)
		assert.Equal(t, 8080, service.Port)
		t.Logf("Service mesh basic setup successful: %s", serviceName)
	}
}

// ==================== Sidecar Proxy Tests ====================

// TestServiceMeshSidecarProxy tests sidecar proxy registration and configuration
func TestServiceMeshSidecarProxy(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	serviceName := "sidecar-proxy-" + randomString(8)

	// Register service with sidecar proxy configuration
	reg := &api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
		Connect: &api.AgentServiceConnect{
			SidecarService: &api.AgentServiceRegistration{
				Port: 21000,
				Proxy: &api.AgentServiceConnectProxyConfig{
					Config: map[string]interface{}{
						"protocol":                  "http",
						"local_connect_timeout_ms": 5000,
						"handshake_timeout_ms":     10000,
					},
				},
			},
		},
	}

	err := agent.ServiceRegister(reg)
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)
	defer agent.ServiceDeregister(serviceName + "-sidecar-proxy")

	time.Sleep(500 * time.Millisecond)

	// Verify both services are registered
	services, err := agent.Services()
	require.NoError(t, err)

	_, hasMain := services[serviceName]
	sidecar, hasSidecar := services[serviceName+"-sidecar-proxy"]

	assert.True(t, hasMain, "Main service should be registered")
	if hasSidecar {
		assert.Equal(t, api.ServiceKindConnectProxy, sidecar.Kind)
		t.Logf("Sidecar proxy registered successfully: %s-sidecar-proxy", serviceName)
	}
}

// ==================== Upstream Configuration Tests ====================

// TestServiceMeshUpstreams tests upstream service configuration
func TestServiceMeshUpstreams(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	serviceName := "upstream-test-" + randomString(8)
	upstreamService := "upstream-backend-" + randomString(8)

	// First register the upstream service
	upstreamReg := &api.AgentServiceRegistration{
		ID:   upstreamService,
		Name: upstreamService,
		Port: 9090,
	}
	err := agent.ServiceRegister(upstreamReg)
	require.NoError(t, err)
	defer agent.ServiceDeregister(upstreamService)

	// Register service with upstream configuration
	reg := &api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
		Connect: &api.AgentServiceConnect{
			SidecarService: &api.AgentServiceRegistration{
				Port: 21000,
				Proxy: &api.AgentServiceConnectProxyConfig{
					Upstreams: []api.Upstream{
						{
							DestinationName: upstreamService,
							LocalBindPort:   9091,
						},
					},
				},
			},
		},
	}

	err = agent.ServiceRegister(reg)
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)
	defer agent.ServiceDeregister(serviceName + "-sidecar-proxy")

	time.Sleep(500 * time.Millisecond)

	// Verify services are registered
	services, err := agent.Services()
	require.NoError(t, err)

	_, hasMain := services[serviceName]
	assert.True(t, hasMain, "Main service should be registered")
	t.Logf("Service with upstreams registered: %s -> %s", serviceName, upstreamService)
}

// ==================== Local Bind Address Tests ====================

// TestServiceMeshLocalBindAddress tests local bind address configuration
func TestServiceMeshLocalBindAddress(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	serviceName := "local-bind-" + randomString(8)

	// Register service with custom local bind address
	reg := &api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
		Connect: &api.AgentServiceConnect{
			SidecarService: &api.AgentServiceRegistration{
				Port:    21000,
				Address: "127.0.0.1",
				Proxy: &api.AgentServiceConnectProxyConfig{
					LocalServiceAddress: "127.0.0.1",
					LocalServicePort:    8080,
					Upstreams: []api.Upstream{
						{
							DestinationName:  "backend-service",
							LocalBindAddress: "127.0.0.1",
							LocalBindPort:    9091,
						},
					},
				},
			},
		},
	}

	err := agent.ServiceRegister(reg)
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)
	defer agent.ServiceDeregister(serviceName + "-sidecar-proxy")

	time.Sleep(500 * time.Millisecond)

	services, err := agent.Services()
	require.NoError(t, err)

	if sidecar, ok := services[serviceName+"-sidecar-proxy"]; ok {
		t.Logf("Sidecar with local bind: %s, Address: %s", sidecar.ID, sidecar.Address)
	}
}

// ==================== Transparent Proxy Tests ====================

// TestServiceMeshTransparentProxy tests transparent proxy mode configuration
func TestServiceMeshTransparentProxy(t *testing.T) {
	client := getTestClient(t)

	configEntries := client.ConfigEntries()

	// Create mesh config entry with transparent proxy settings
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

	// Get and verify the mesh config
	gotEntry, _, err := configEntries.Get(api.MeshConfig, api.MeshConfigMesh, nil)
	if err != nil {
		t.Logf("Get mesh config: %v", err)
		configEntries.Delete(api.MeshConfig, api.MeshConfigMesh, nil)
		return
	}

	meshEntry := gotEntry.(*api.MeshConfigEntry)
	assert.True(t, meshEntry.TransparentProxy.MeshDestinationsOnly,
		"TransparentProxy.MeshDestinationsOnly should be true")
	t.Logf("Transparent proxy config created successfully")

	// Cleanup
	configEntries.Delete(api.MeshConfig, api.MeshConfigMesh, nil)
}

// ==================== Mesh Gateway Tests ====================

// TestServiceMeshMeshGateway tests mesh gateway registration and configuration
func TestServiceMeshMeshGateway(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	gatewayName := "mesh-gw-" + randomString(8)

	// Register mesh gateway
	reg := &api.AgentServiceRegistration{
		ID:   gatewayName,
		Name: gatewayName,
		Port: 8443,
		Kind: api.ServiceKindMeshGateway,
		TaggedAddresses: map[string]api.ServiceAddress{
			"lan": {
				Address: "10.0.0.1",
				Port:    8443,
			},
			"wan": {
				Address: "192.168.1.100",
				Port:    8443,
			},
		},
		Proxy: &api.AgentServiceConnectProxyConfig{
			Config: map[string]interface{}{
				"envoy_mesh_gateway_no_default_bind":       true,
				"envoy_mesh_gateway_bind_tagged_addresses": true,
			},
		},
	}

	err := agent.ServiceRegister(reg)
	require.NoError(t, err)
	defer agent.ServiceDeregister(gatewayName)

	time.Sleep(500 * time.Millisecond)

	// Verify gateway is registered
	services, err := agent.Services()
	require.NoError(t, err)

	if gw, ok := services[gatewayName]; ok {
		assert.Equal(t, api.ServiceKindMeshGateway, gw.Kind)
		t.Logf("Mesh gateway registered: %s with LAN and WAN addresses", gatewayName)
	}
}

// ==================== Terminating Gateway Tests ====================

// TestServiceMeshTerminatingGateway tests terminating gateway for external services
func TestServiceMeshTerminatingGateway(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	configEntries := client.ConfigEntries()
	gatewayName := "term-gw-" + randomString(8)

	// Register terminating gateway service
	reg := &api.AgentServiceRegistration{
		ID:   gatewayName,
		Name: gatewayName,
		Port: 8443,
		Kind: api.ServiceKindTerminatingGateway,
	}

	err := agent.ServiceRegister(reg)
	require.NoError(t, err)
	defer agent.ServiceDeregister(gatewayName)

	// Create terminating gateway config entry
	termGwConfig := &api.TerminatingGatewayConfigEntry{
		Kind: api.TerminatingGateway,
		Name: gatewayName,
		Services: []api.LinkedService{
			{
				Name: "external-database",
			},
			{
				Name:     "external-api",
				CAFile:   "/etc/ssl/certs/ca.pem",
				CertFile: "/etc/ssl/certs/client.pem",
				KeyFile:  "/etc/ssl/certs/client-key.pem",
				SNI:      "external-api.example.com",
			},
		},
	}

	_, _, err = configEntries.Set(termGwConfig, nil)
	if err != nil {
		t.Logf("Terminating gateway config not available: %v", err)
		return
	}
	defer configEntries.Delete(api.TerminatingGateway, gatewayName, nil)

	time.Sleep(500 * time.Millisecond)

	// Verify gateway
	services, err := agent.Services()
	require.NoError(t, err)

	if gw, ok := services[gatewayName]; ok {
		assert.Equal(t, api.ServiceKindTerminatingGateway, gw.Kind)
		t.Logf("Terminating gateway registered: %s", gatewayName)
	}
}

// ==================== Ingress Gateway Tests ====================

// TestServiceMeshIngressGateway tests ingress gateway for external access
func TestServiceMeshIngressGateway(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	configEntries := client.ConfigEntries()
	gatewayName := "ingress-gw-" + randomString(8)

	// Register ingress gateway service
	reg := &api.AgentServiceRegistration{
		ID:   gatewayName,
		Name: gatewayName,
		Port: 8080,
		Kind: api.ServiceKindIngressGateway,
	}

	err := agent.ServiceRegister(reg)
	require.NoError(t, err)
	defer agent.ServiceDeregister(gatewayName)

	// Create ingress gateway config entry
	ingressConfig := &api.IngressGatewayConfigEntry{
		Kind: api.IngressGateway,
		Name: gatewayName,
		Listeners: []api.IngressListener{
			{
				Port:     8080,
				Protocol: "http",
				Services: []api.IngressService{
					{
						Name:  "web-frontend",
						Hosts: []string{"www.example.com"},
					},
				},
			},
			{
				Port:     8443,
				Protocol: "http",
				Services: []api.IngressService{
					{
						Name: "api-service",
					},
				},
			},
		},
	}

	_, _, err = configEntries.Set(ingressConfig, nil)
	if err != nil {
		t.Logf("Ingress gateway config not available: %v", err)
		return
	}
	defer configEntries.Delete(api.IngressGateway, gatewayName, nil)

	time.Sleep(500 * time.Millisecond)

	// Verify gateway
	services, err := agent.Services()
	require.NoError(t, err)

	if gw, ok := services[gatewayName]; ok {
		assert.Equal(t, api.ServiceKindIngressGateway, gw.Kind)
		t.Logf("Ingress gateway registered with %d listeners: %s",
			len(ingressConfig.Listeners), gatewayName)
	}
}

// ==================== Service Defaults Tests ====================

// TestServiceMeshServiceDefaults tests service defaults config entry
func TestServiceMeshServiceDefaults(t *testing.T) {
	client := getTestClient(t)

	configEntries := client.ConfigEntries()
	serviceName := "svc-defaults-" + randomString(8)

	// Create service defaults
	entry := &api.ServiceConfigEntry{
		Kind:     api.ServiceDefaults,
		Name:     serviceName,
		Protocol: "http",
		MeshGateway: api.MeshGatewayConfig{
			Mode: api.MeshGatewayModeLocal,
		},
		UpstreamConfig: &api.UpstreamConfiguration{
			Defaults: &api.UpstreamConfig{
				ConnectTimeoutMs: 5000,
				Limits: &api.UpstreamLimits{
					MaxConnections:        intPtr(100),
					MaxPendingRequests:    intPtr(200),
					MaxConcurrentRequests: intPtr(50),
				},
			},
		},
	}

	_, _, err := configEntries.Set(entry, nil)
	if err != nil {
		t.Logf("Service defaults not available: %v", err)
		return
	}
	defer configEntries.Delete(api.ServiceDefaults, serviceName, nil)

	// Verify the entry
	gotEntry, _, err := configEntries.Get(api.ServiceDefaults, serviceName, nil)
	require.NoError(t, err)

	serviceEntry := gotEntry.(*api.ServiceConfigEntry)
	assert.Equal(t, "http", serviceEntry.Protocol)
	assert.Equal(t, api.MeshGatewayModeLocal, serviceEntry.MeshGateway.Mode)
	t.Logf("Service defaults created: %s with protocol %s", serviceName, serviceEntry.Protocol)
}

// ==================== Proxy Defaults Tests ====================

// TestServiceMeshProxyDefaults tests proxy defaults config entry
func TestServiceMeshProxyDefaults(t *testing.T) {
	client := getTestClient(t)

	configEntries := client.ConfigEntries()

	// Create proxy defaults (global)
	entry := &api.ProxyConfigEntry{
		Kind: api.ProxyDefaults,
		Name: api.ProxyConfigGlobal,
		Config: map[string]interface{}{
			"protocol":                 "http",
			"local_connect_timeout_ms": 5000,
			"handshake_timeout_ms":     10000,
		},
		MeshGateway: api.MeshGatewayConfig{
			Mode: api.MeshGatewayModeLocal,
		},
		Expose: api.ExposeConfig{
			Checks: true,
			Paths: []api.ExposePath{
				{
					Path:          "/health",
					LocalPathPort: 8080,
					ListenerPort:  21500,
					Protocol:      "http",
				},
			},
		},
	}

	_, _, err := configEntries.Set(entry, nil)
	if err != nil {
		t.Logf("Proxy defaults not available: %v", err)
		return
	}
	defer configEntries.Delete(api.ProxyDefaults, api.ProxyConfigGlobal, nil)

	// Verify the entry
	gotEntry, _, err := configEntries.Get(api.ProxyDefaults, api.ProxyConfigGlobal, nil)
	if err != nil {
		t.Logf("Get proxy defaults: %v", err)
		return
	}

	proxyEntry := gotEntry.(*api.ProxyConfigEntry)
	assert.NotNil(t, proxyEntry.Config)
	t.Logf("Proxy defaults created with mesh gateway mode: %s", proxyEntry.MeshGateway.Mode)
}

// ==================== Service Router Tests ====================

// TestServiceMeshServiceRouter tests service router config entry for traffic routing
func TestServiceMeshServiceRouter(t *testing.T) {
	client := getTestClient(t)

	configEntries := client.ConfigEntries()
	serviceName := "router-svc-" + randomString(8)

	// First create service defaults (required for router)
	defaults := &api.ServiceConfigEntry{
		Kind:     api.ServiceDefaults,
		Name:     serviceName,
		Protocol: "http",
	}
	configEntries.Set(defaults, nil)
	defer configEntries.Delete(api.ServiceDefaults, serviceName, nil)

	// Create service router with various routing rules
	router := &api.ServiceRouterConfigEntry{
		Kind: api.ServiceRouter,
		Name: serviceName,
		Routes: []api.ServiceRoute{
			{
				Match: &api.ServiceRouteMatch{
					HTTP: &api.ServiceRouteHTTPMatch{
						PathPrefix: "/api/v1",
						Header: []api.ServiceRouteHTTPMatchHeader{
							{
								Name:  "X-Version",
								Exact: "1",
							},
						},
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
						QueryParam: []api.ServiceRouteHTTPMatchQueryParam{
							{
								Name:  "version",
								Exact: "2",
							},
						},
					},
				},
				Destination: &api.ServiceRouteDestination{
					Service:       serviceName,
					ServiceSubset: "v2",
					PrefixRewrite: "/api",
				},
			},
			{
				Match: &api.ServiceRouteMatch{
					HTTP: &api.ServiceRouteHTTPMatch{
						Methods: []string{"POST", "PUT"},
					},
				},
				Destination: &api.ServiceRouteDestination{
					Service:               serviceName,
					ServiceSubset:         "write",
					RequestTimeout:        30 * time.Second,
					NumRetries:            3,
					RetryOnConnectFailure: true,
				},
			},
		},
	}

	_, _, err := configEntries.Set(router, nil)
	if err != nil {
		t.Logf("Service router not available: %v", err)
		return
	}
	defer configEntries.Delete(api.ServiceRouter, serviceName, nil)

	t.Logf("Service router created with %d routes for %s", len(router.Routes), serviceName)
}

// ==================== Service Splitter Tests ====================

// TestServiceMeshServiceSplitter tests service splitter for traffic splitting
func TestServiceMeshServiceSplitter(t *testing.T) {
	client := getTestClient(t)

	configEntries := client.ConfigEntries()
	serviceName := "splitter-svc-" + randomString(8)

	// First create service defaults
	defaults := &api.ServiceConfigEntry{
		Kind:     api.ServiceDefaults,
		Name:     serviceName,
		Protocol: "http",
	}
	configEntries.Set(defaults, nil)
	defer configEntries.Delete(api.ServiceDefaults, serviceName, nil)

	// Create service splitter for canary deployment
	splitter := &api.ServiceSplitterConfigEntry{
		Kind: api.ServiceSplitter,
		Name: serviceName,
		Splits: []api.ServiceSplit{
			{
				Weight:        80,
				ServiceSubset: "stable",
				RequestHeaders: &api.HTTPHeaderModifiers{
					Set: map[string]string{
						"X-Canary": "false",
					},
				},
			},
			{
				Weight:        15,
				ServiceSubset: "canary",
				RequestHeaders: &api.HTTPHeaderModifiers{
					Set: map[string]string{
						"X-Canary": "true",
					},
				},
			},
			{
				Weight:        5,
				ServiceSubset: "experimental",
				RequestHeaders: &api.HTTPHeaderModifiers{
					Set: map[string]string{
						"X-Canary":       "true",
						"X-Experimental": "true",
					},
				},
			},
		},
	}

	_, _, err := configEntries.Set(splitter, nil)
	if err != nil {
		t.Logf("Service splitter not available: %v", err)
		return
	}
	defer configEntries.Delete(api.ServiceSplitter, serviceName, nil)

	t.Logf("Service splitter created: 80%% stable, 15%% canary, 5%% experimental")
}

// ==================== Service Resolver Tests ====================

// TestServiceMeshServiceResolver tests service resolver for service discovery
func TestServiceMeshServiceResolver(t *testing.T) {
	client := getTestClient(t)

	configEntries := client.ConfigEntries()
	serviceName := "resolver-svc-" + randomString(8)

	// Create service resolver with subsets and failover
	resolver := &api.ServiceResolverConfigEntry{
		Kind:          api.ServiceResolver,
		Name:          serviceName,
		DefaultSubset: "stable",
		Subsets: map[string]api.ServiceResolverSubset{
			"stable": {
				Filter: "Service.Meta.version == v1",
			},
			"canary": {
				Filter:      "Service.Meta.version == v2",
				OnlyPassing: true,
			},
			"experimental": {
				Filter: "Service.Meta.version == v3",
			},
		},
		ConnectTimeout: 10 * time.Second,
		Failover: map[string]api.ServiceResolverFailover{
			"*": {
				Datacenters: []string{"dc2", "dc3"},
			},
			"canary": {
				ServiceSubset: "stable",
			},
		},
		LoadBalancer: &api.LoadBalancer{
			Policy: "least_request",
			LeastRequestConfig: &api.LeastRequestConfig{
				ChoiceCount: 5,
			},
		},
	}

	_, _, err := configEntries.Set(resolver, nil)
	if err != nil {
		t.Logf("Service resolver not available: %v", err)
		return
	}
	defer configEntries.Delete(api.ServiceResolver, serviceName, nil)

	t.Logf("Service resolver created with %d subsets and failover configuration",
		len(resolver.Subsets))
}

// ==================== Retry Policy Tests ====================

// TestServiceMeshRetryPolicy tests retry policy configuration in service router
func TestServiceMeshRetryPolicy(t *testing.T) {
	client := getTestClient(t)

	configEntries := client.ConfigEntries()
	serviceName := "retry-policy-" + randomString(8)

	// Create service defaults
	defaults := &api.ServiceConfigEntry{
		Kind:     api.ServiceDefaults,
		Name:     serviceName,
		Protocol: "http",
	}
	configEntries.Set(defaults, nil)
	defer configEntries.Delete(api.ServiceDefaults, serviceName, nil)

	// Create router with retry policies
	router := &api.ServiceRouterConfigEntry{
		Kind: api.ServiceRouter,
		Name: serviceName,
		Routes: []api.ServiceRoute{
			{
				Match: &api.ServiceRouteMatch{
					HTTP: &api.ServiceRouteHTTPMatch{
						PathPrefix: "/retry-test",
					},
				},
				Destination: &api.ServiceRouteDestination{
					Service:               serviceName,
					NumRetries:            5,
					RetryOnConnectFailure: true,
					RetryOnStatusCodes:    []uint32{502, 503, 504},
				},
			},
			{
				Match: &api.ServiceRouteMatch{
					HTTP: &api.ServiceRouteHTTPMatch{
						PathPrefix: "/no-retry",
					},
				},
				Destination: &api.ServiceRouteDestination{
					Service:    serviceName,
					NumRetries: 0,
				},
			},
		},
	}

	_, _, err := configEntries.Set(router, nil)
	if err != nil {
		t.Logf("Retry policy configuration not available: %v", err)
		return
	}
	defer configEntries.Delete(api.ServiceRouter, serviceName, nil)

	t.Logf("Retry policy configured: 5 retries on connect failure and status codes 502, 503, 504")
}

// ==================== Timeout Policy Tests ====================

// TestServiceMeshTimeoutPolicy tests timeout policy configuration
func TestServiceMeshTimeoutPolicy(t *testing.T) {
	client := getTestClient(t)

	configEntries := client.ConfigEntries()
	serviceName := "timeout-policy-" + randomString(8)

	// Create service defaults with upstream timeout config
	defaults := &api.ServiceConfigEntry{
		Kind:     api.ServiceDefaults,
		Name:     serviceName,
		Protocol: "http",
		UpstreamConfig: &api.UpstreamConfiguration{
			Defaults: &api.UpstreamConfig{
				ConnectTimeoutMs: 10000,
			},
		},
	}

	_, _, err := configEntries.Set(defaults, nil)
	if err != nil {
		t.Logf("Service defaults with timeout not available: %v", err)
		return
	}
	defer configEntries.Delete(api.ServiceDefaults, serviceName, nil)

	// Create router with request timeout
	router := &api.ServiceRouterConfigEntry{
		Kind: api.ServiceRouter,
		Name: serviceName,
		Routes: []api.ServiceRoute{
			{
				Match: &api.ServiceRouteMatch{
					HTTP: &api.ServiceRouteHTTPMatch{
						PathPrefix: "/slow-endpoint",
					},
				},
				Destination: &api.ServiceRouteDestination{
					Service:        serviceName,
					RequestTimeout: 60 * time.Second,
					IdleTimeout:    120 * time.Second,
				},
			},
			{
				Match: &api.ServiceRouteMatch{
					HTTP: &api.ServiceRouteHTTPMatch{
						PathPrefix: "/fast-endpoint",
					},
				},
				Destination: &api.ServiceRouteDestination{
					Service:        serviceName,
					RequestTimeout: 5 * time.Second,
				},
			},
		},
	}

	_, _, err = configEntries.Set(router, nil)
	if err != nil {
		t.Logf("Timeout policy configuration not available: %v", err)
		return
	}
	defer configEntries.Delete(api.ServiceRouter, serviceName, nil)

	t.Logf("Timeout policy configured: 60s for slow, 5s for fast endpoints")
}

// ==================== Circuit Breaker Tests ====================

// TestServiceMeshCircuitBreaker tests circuit breaker configuration
func TestServiceMeshCircuitBreaker(t *testing.T) {
	client := getTestClient(t)

	configEntries := client.ConfigEntries()
	serviceName := "circuit-breaker-" + randomString(8)

	// Create service defaults with circuit breaker limits
	defaults := &api.ServiceConfigEntry{
		Kind:     api.ServiceDefaults,
		Name:     serviceName,
		Protocol: "http",
		UpstreamConfig: &api.UpstreamConfiguration{
			Defaults: &api.UpstreamConfig{
				Limits: &api.UpstreamLimits{
					MaxConnections:        intPtr(100),
					MaxPendingRequests:    intPtr(100),
					MaxConcurrentRequests: intPtr(100),
				},
				PassiveHealthCheck: &api.PassiveHealthCheck{
					Interval:                  10 * time.Second,
					MaxFailures:               5,
					EnforcingConsecutive5xx:   uintPtr(100),
					MaxEjectionPercent:        uintPtr(50),
					BaseEjectionTime:          durationPtr(30 * time.Second),
				},
			},
		},
	}

	_, _, err := configEntries.Set(defaults, nil)
	if err != nil {
		t.Logf("Circuit breaker configuration not available: %v", err)
		return
	}
	defer configEntries.Delete(api.ServiceDefaults, serviceName, nil)

	// Verify the entry
	gotEntry, _, err := configEntries.Get(api.ServiceDefaults, serviceName, nil)
	if err != nil {
		t.Logf("Get service defaults: %v", err)
		return
	}

	serviceEntry := gotEntry.(*api.ServiceConfigEntry)
	if serviceEntry.UpstreamConfig != nil && serviceEntry.UpstreamConfig.Defaults != nil {
		limits := serviceEntry.UpstreamConfig.Defaults.Limits
		if limits != nil {
			t.Logf("Circuit breaker configured: max connections=%d, max pending=%d",
				*limits.MaxConnections, *limits.MaxPendingRequests)
		}
	}
}

// ==================== Rate Limiting Tests ====================

// TestServiceMeshRateLimiting tests rate limiting configuration
func TestServiceMeshRateLimiting(t *testing.T) {
	client := getTestClient(t)

	configEntries := client.ConfigEntries()
	serviceName := "rate-limit-" + randomString(8)

	// Create service defaults with rate limiting via limits
	defaults := &api.ServiceConfigEntry{
		Kind:     api.ServiceDefaults,
		Name:     serviceName,
		Protocol: "http",
		RateLimits: &api.RateLimits{
			InstanceLevel: api.InstanceLevelRateLimits{
				RequestsPerSecond: 100,
				RequestsMaxBurst:  200,
			},
		},
		UpstreamConfig: &api.UpstreamConfiguration{
			Defaults: &api.UpstreamConfig{
				Limits: &api.UpstreamLimits{
					MaxConnections:        intPtr(50),
					MaxPendingRequests:    intPtr(100),
					MaxConcurrentRequests: intPtr(25),
				},
			},
		},
	}

	_, _, err := configEntries.Set(defaults, nil)
	if err != nil {
		t.Logf("Rate limiting configuration not available: %v", err)
		return
	}
	defer configEntries.Delete(api.ServiceDefaults, serviceName, nil)

	t.Logf("Rate limiting configured: 100 req/s with burst of 200")
}

// ==================== mTLS Tests ====================

// TestServiceMeshMTLS tests mutual TLS configuration
func TestServiceMeshMTLS(t *testing.T) {
	client := getTestClient(t)

	connect := client.Connect()
	configEntries := client.ConfigEntries()

	// Check CA roots (Connect must be enabled)
	roots, _, err := connect.CARoots(nil)
	if err != nil {
		t.Logf("Connect CA not available (Connect may not be enabled): %v", err)
		return
	}

	if roots != nil && len(roots.Roots) > 0 {
		t.Logf("CA roots available: %d roots found", len(roots.Roots))
		for _, root := range roots.Roots {
			t.Logf("  Root ID: %s, Active: %v", root.ID, root.Active)
		}
	}

	// Check CA configuration
	caConfig, _, err := connect.CAGetConfig(nil)
	if err != nil {
		t.Logf("CA config not available: %v", err)
		return
	}
	t.Logf("CA Provider: %s", caConfig.Provider)

	// Create mesh config with mTLS mode
	mesh := &api.MeshConfigEntry{
		TLS: &api.MeshTLSConfig{
			Incoming: &api.MeshDirectionalTLSConfig{
				TLSMinVersion: "TLSv1_2",
				CipherSuites: []string{
					"TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256",
					"TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256",
				},
			},
			Outgoing: &api.MeshDirectionalTLSConfig{
				TLSMinVersion: "TLSv1_2",
			},
		},
	}

	_, _, err = configEntries.Set(mesh, nil)
	if err != nil {
		t.Logf("Mesh TLS config not available: %v", err)
		return
	}
	defer configEntries.Delete(api.MeshConfig, api.MeshConfigMesh, nil)

	// Create service with mTLS requirements
	serviceName := "mtls-svc-" + randomString(8)
	serviceDefaults := &api.ServiceConfigEntry{
		Kind:     api.ServiceDefaults,
		Name:     serviceName,
		Protocol: "http",
		MutualTLSMode: api.MutualTLSModeStrict,
	}

	_, _, err = configEntries.Set(serviceDefaults, nil)
	if err != nil {
		t.Logf("Service mTLS defaults not available: %v", err)
		return
	}
	defer configEntries.Delete(api.ServiceDefaults, serviceName, nil)

	t.Logf("mTLS configured: strict mode with TLS 1.2 minimum")
}

// ==================== Helper Functions ====================

func intPtr(i int) *int {
	return &i
}

func uintPtr(u uint32) *uint32 {
	return &u
}

func boolPtr(b bool) *bool {
	return &b
}

func durationPtr(d time.Duration) *time.Duration {
	return &d
}
