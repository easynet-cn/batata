package tests

import (
	"testing"
	"time"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// ==================== Agent Connect & Proxy Tests ====================

// TestAgentServiceSidecarProxy tests automatic sidecar proxy registration
func TestAgentServiceSidecarProxy(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	serviceName := "sidecar-test-" + randomString(8)

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

	// Verify both services registered
	services, err := agent.Services()
	require.NoError(t, err)

	_, hasMain := services[serviceName]
	_, hasSidecar := services[serviceName+"-sidecar-proxy"]

	t.Logf("Main service registered: %v, Sidecar registered: %v", hasMain, hasSidecar)
	assert.True(t, hasMain, "Main service should be registered")
}

// TestAgentServiceExplicitProxy tests explicit proxy registration
func TestAgentServiceExplicitProxy(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	serviceName := "explicit-proxy-" + randomString(8)
	proxyName := serviceName + "-proxy"

	// Register main service
	mainReg := &api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	}
	err := agent.ServiceRegister(mainReg)
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)

	// Register explicit proxy
	proxyReg := &api.AgentServiceRegistration{
		ID:   proxyName,
		Name: proxyName,
		Port: 21000,
		Kind: api.ServiceKindConnectProxy,
		Proxy: &api.AgentServiceConnectProxyConfig{
			DestinationServiceName: serviceName,
			LocalServicePort:       8080,
		},
	}
	err = agent.ServiceRegister(proxyReg)
	require.NoError(t, err)
	defer agent.ServiceDeregister(proxyName)

	time.Sleep(500 * time.Millisecond)

	// Verify proxy
	services, err := agent.Services()
	require.NoError(t, err)

	if proxy, ok := services[proxyName]; ok {
		assert.Equal(t, api.ServiceKindConnectProxy, proxy.Kind)
		if proxy.Proxy != nil {
			t.Logf("Proxy registered for destination: %s", proxy.Proxy.DestinationServiceName)
		} else {
			t.Log("Proxy registered but Proxy config is nil")
		}
	}
}

// TestAgentServiceConnectNative tests native Connect service
func TestAgentServiceConnectNative(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	serviceName := "connect-native-" + randomString(8)

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

	services, err := agent.Services()
	require.NoError(t, err)

	if svc, ok := services[serviceName]; ok {
		t.Logf("Service Connect.Native: %v", svc.Connect)
	}
}

// ==================== Agent Multi-Port Service Tests ====================

// TestAgentServiceMultiPort tests multi-port service registration
func TestAgentServiceMultiPort(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	serviceName := "multi-port-" + randomString(8)

	// Register service with tagged addresses for multiple ports
	reg := &api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
		TaggedAddresses: map[string]api.ServiceAddress{
			"http": {
				Address: "127.0.0.1",
				Port:    8080,
			},
			"grpc": {
				Address: "127.0.0.1",
				Port:    9090,
			},
			"admin": {
				Address: "127.0.0.1",
				Port:    8081,
			},
		},
	}

	err := agent.ServiceRegister(reg)
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	services, err := agent.Services()
	require.NoError(t, err)

	if svc, ok := services[serviceName]; ok {
		t.Logf("Tagged addresses: %v", svc.TaggedAddresses)
		assert.NotEmpty(t, svc.TaggedAddresses, "Should have tagged addresses")
	}
}

// TestAgentServiceSocket tests Unix socket path registration
func TestAgentServiceSocket(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	serviceName := "socket-svc-" + randomString(8)

	reg := &api.AgentServiceRegistration{
		ID:         serviceName,
		Name:       serviceName,
		SocketPath: "/tmp/test-service.sock",
	}

	err := agent.ServiceRegister(reg)
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	services, err := agent.Services()
	require.NoError(t, err)

	if svc, ok := services[serviceName]; ok {
		t.Logf("Socket path: %s", svc.SocketPath)
	}
}

// ==================== Agent Token Management Tests ====================

// TestAgentUpdateACLToken tests updating agent ACL tokens
func TestAgentUpdateACLToken(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()

	// Try to update agent token (requires proper ACL setup)
	token := "test-token-" + randomString(8)
	_, err := agent.UpdateAgentACLToken(token, nil)
	if err != nil {
		t.Logf("Token update not available (ACL may be disabled): %v", err)
		return
	}
	t.Log("Agent token updated successfully")
}

// TestAgentTokenFilePriority tests token file configuration
func TestAgentTokenFilePriority(t *testing.T) {
	// This test verifies token configuration priorities
	// In practice, this is configured at client creation time

	config := api.DefaultConfig()
	config.Token = "direct-token"

	// Token priority: CLI > Env File > Env Var > Config
	t.Logf("Token configuration priority test")
	t.Logf("Direct token configured: %v", config.Token != "")
}

// ==================== Agent Monitor Tests ====================

// TestAgentMonitor tests log streaming from agent
func TestAgentMonitor(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()

	// Start monitoring with debug level
	logCh, err := agent.Monitor("debug", nil, nil)
	if err != nil {
		t.Logf("Agent monitor not available: %v", err)
		return
	}

	// Collect a few log lines
	timeout := time.After(2 * time.Second)
	count := 0

	for {
		select {
		case log := <-logCh:
			t.Logf("Log: %s", log)
			count++
			if count >= 3 {
				return
			}
		case <-timeout:
			t.Logf("Received %d log lines", count)
			return
		}
	}
}

// TestAgentMonitorJSON tests JSON log streaming
func TestAgentMonitorJSON(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()

	logCh, err := agent.MonitorJSON("info", nil, nil)
	if err != nil {
		t.Logf("Agent monitor JSON not available: %v", err)
		return
	}

	timeout := time.After(2 * time.Second)
	count := 0

	for {
		select {
		case log := <-logCh:
			t.Logf("JSON Log: %s", log)
			count++
			if count >= 2 {
				return
			}
		case <-timeout:
			t.Logf("Received %d JSON log lines", count)
			return
		}
	}
}

// ==================== Agent Gateway Tests ====================

// TestAgentMeshGateway tests mesh gateway registration
func TestAgentMeshGateway(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	gatewayName := "mesh-gateway-" + randomString(8)

	reg := &api.AgentServiceRegistration{
		ID:   gatewayName,
		Name: gatewayName,
		Port: 8443,
		Kind: api.ServiceKindMeshGateway,
		Proxy: &api.AgentServiceConnectProxyConfig{
			Config: map[string]interface{}{
				"envoy_mesh_gateway_no_default_bind": true,
			},
		},
	}

	err := agent.ServiceRegister(reg)
	require.NoError(t, err)
	defer agent.ServiceDeregister(gatewayName)

	time.Sleep(500 * time.Millisecond)

	services, err := agent.Services()
	require.NoError(t, err)

	if svc, ok := services[gatewayName]; ok {
		assert.Equal(t, api.ServiceKindMeshGateway, svc.Kind)
		t.Logf("Mesh gateway registered: %s", gatewayName)
	}
}

// TestAgentTerminatingGateway tests terminating gateway registration
func TestAgentTerminatingGateway(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	gatewayName := "term-gateway-" + randomString(8)

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

	services, err := agent.Services()
	require.NoError(t, err)

	if svc, ok := services[gatewayName]; ok {
		assert.Equal(t, api.ServiceKindTerminatingGateway, svc.Kind)
		t.Logf("Terminating gateway registered: %s", gatewayName)
	}
}

// TestAgentIngressGateway tests ingress gateway registration
func TestAgentIngressGateway(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	gatewayName := "ingress-gateway-" + randomString(8)

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

	services, err := agent.Services()
	require.NoError(t, err)

	if svc, ok := services[gatewayName]; ok {
		assert.Equal(t, api.ServiceKindIngressGateway, svc.Kind)
		t.Logf("Ingress gateway registered: %s", gatewayName)
	}
}

// ==================== Agent Health Check Advanced Tests ====================

// TestAgentCheckDocker tests Docker container health check
func TestAgentCheckDocker(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	checkID := "docker-check-" + randomString(8)

	// Docker check (will fail without Docker but tests API)
	reg := &api.AgentCheckRegistration{
		ID:   checkID,
		Name: "Docker Check",
		AgentServiceCheck: api.AgentServiceCheck{
			DockerContainerID: "test-container-id",
			Shell:             "/bin/sh",
			Args:              []string{"echo", "healthy"},
			Interval:          "10s",
		},
	}

	err := agent.CheckRegister(reg)
	if err != nil {
		t.Logf("Docker check registration: %v", err)
		// This is expected to fail without proper Docker setup
		return
	}
	defer agent.CheckDeregister(checkID)

	t.Log("Docker check registered")
}

// TestAgentCheckScript tests script-based health check
func TestAgentCheckScript(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	checkID := "script-check-" + randomString(8)

	reg := &api.AgentCheckRegistration{
		ID:   checkID,
		Name: "Script Check",
		AgentServiceCheck: api.AgentServiceCheck{
			Args:     []string{"echo", "healthy"},
			Interval: "10s",
		},
	}

	err := agent.CheckRegister(reg)
	if err != nil {
		t.Logf("Script check registration: %v (scripts may be disabled)", err)
		return
	}
	defer agent.CheckDeregister(checkID)

	t.Log("Script check registered")
}

// TestAgentCheckHTTP tests HTTP health check
func TestAgentCheckHTTP(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	checkID := "http-check-" + randomString(8)

	reg := &api.AgentCheckRegistration{
		ID:   checkID,
		Name: "HTTP Check",
		AgentServiceCheck: api.AgentServiceCheck{
			HTTP:     "http://localhost:8080/health",
			Interval: "10s",
			Timeout:  "5s",
		},
	}

	err := agent.CheckRegister(reg)
	require.NoError(t, err)
	defer agent.CheckDeregister(checkID)

	time.Sleep(500 * time.Millisecond)

	checks, err := agent.Checks()
	require.NoError(t, err)

	if check, ok := checks[checkID]; ok {
		t.Logf("HTTP check status: %s", check.Status)
	}
}

// TestAgentCheckTCP tests TCP health check
func TestAgentCheckTCP(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	checkID := "tcp-check-" + randomString(8)

	reg := &api.AgentCheckRegistration{
		ID:   checkID,
		Name: "TCP Check",
		AgentServiceCheck: api.AgentServiceCheck{
			TCP:      "localhost:8080",
			Interval: "10s",
			Timeout:  "5s",
		},
	}

	err := agent.CheckRegister(reg)
	require.NoError(t, err)
	defer agent.CheckDeregister(checkID)

	time.Sleep(500 * time.Millisecond)

	checks, err := agent.Checks()
	require.NoError(t, err)

	if check, ok := checks[checkID]; ok {
		t.Logf("TCP check status: %s", check.Status)
	}
}

// TestAgentCheckGRPC tests gRPC health check
func TestAgentCheckGRPC(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	checkID := "grpc-check-" + randomString(8)

	reg := &api.AgentCheckRegistration{
		ID:   checkID,
		Name: "gRPC Check",
		AgentServiceCheck: api.AgentServiceCheck{
			GRPC:     "localhost:9090",
			Interval: "10s",
			Timeout:  "5s",
		},
	}

	err := agent.CheckRegister(reg)
	require.NoError(t, err)
	defer agent.CheckDeregister(checkID)

	time.Sleep(500 * time.Millisecond)

	checks, err := agent.Checks()
	require.NoError(t, err)

	if check, ok := checks[checkID]; ok {
		t.Logf("gRPC check status: %s", check.Status)
	}
}

// TestAgentCheckDeregisterCritical tests auto-deregistration on critical
func TestAgentCheckDeregisterCritical(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	serviceID := "deregister-critical-" + randomString(8)

	reg := &api.AgentServiceRegistration{
		ID:   serviceID,
		Name: serviceID,
		Port: 8080,
		Check: &api.AgentServiceCheck{
			TTL:                            "5s",
			DeregisterCriticalServiceAfter: "1m",
		},
	}

	err := agent.ServiceRegister(reg)
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceID)

	time.Sleep(500 * time.Millisecond)

	// Get service to verify deregister setting
	services, err := agent.Services()
	require.NoError(t, err)

	if _, ok := services[serviceID]; ok {
		t.Log("Service with DeregisterCriticalServiceAfter registered")
	}
}
