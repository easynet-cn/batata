package tests

import (
	"fmt"
	"os"
	"testing"
	"time"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func getClient(t *testing.T) *api.Client {
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

func randomID() string {
	return fmt.Sprintf("%d", time.Now().UnixNano())
}

// ==================== P0: Critical Tests ====================

// CA-001: Test service registration
func TestAgentServiceRegister(t *testing.T) {
	client := getClient(t)
	serviceID := "ca001-register-" + randomID()

	registration := &api.AgentServiceRegistration{
		ID:      serviceID,
		Name:    "test-service",
		Port:    8080,
		Address: "192.168.1.100",
		Tags:    []string{"test", "integration"},
		Meta: map[string]string{
			"version": "1.0.0",
		},
	}

	// Register
	err := client.Agent().ServiceRegister(registration)
	assert.NoError(t, err, "Service registration should succeed")

	// Wait for registration
	time.Sleep(500 * time.Millisecond)

	// Verify
	services, err := client.Agent().Services()
	assert.NoError(t, err)
	assert.Contains(t, services, serviceID, "Service should be registered")

	service, exists := services[serviceID]
	if assert.True(t, exists, "Service should exist in map") {
		assert.Equal(t, "test-service", service.Service)
		assert.Equal(t, 8080, service.Port)
		assert.Equal(t, "192.168.1.100", service.Address)
		assert.Contains(t, service.Tags, "test")
		assert.Equal(t, "1.0.0", service.Meta["version"])
	}

	// Cleanup
	err = client.Agent().ServiceDeregister(serviceID)
	assert.NoError(t, err)
}

// CA-002: Test service deregistration
func TestAgentServiceDeregister(t *testing.T) {
	client := getClient(t)
	serviceID := "ca002-deregister-" + randomID()

	// Register first
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceID,
		Name: "to-deregister",
		Port: 8080,
	})
	require.NoError(t, err)
	time.Sleep(500 * time.Millisecond)

	// Verify registered
	services, err := client.Agent().Services()
	require.NoError(t, err)
	require.Contains(t, services, serviceID)

	// Deregister
	err = client.Agent().ServiceDeregister(serviceID)
	assert.NoError(t, err, "Service deregistration should succeed")

	time.Sleep(500 * time.Millisecond)

	// Verify deregistered
	services, err = client.Agent().Services()
	assert.NoError(t, err)
	assert.NotContains(t, services, serviceID, "Service should be deregistered")
}

// CA-003: Test list services
func TestAgentListServices(t *testing.T) {
	client := getClient(t)
	prefix := "ca003-list-" + randomID()

	// Register multiple services
	for i := 0; i < 3; i++ {
		err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
			ID:   fmt.Sprintf("%s-%d", prefix, i),
			Name: fmt.Sprintf("list-service-%d", i),
			Port: 8080 + i,
		})
		require.NoError(t, err)
	}
	time.Sleep(1 * time.Second)

	// List services
	services, err := client.Agent().Services()
	assert.NoError(t, err)
	assert.NotEmpty(t, services)

	// Verify our services exist
	for i := 0; i < 3; i++ {
		serviceID := fmt.Sprintf("%s-%d", prefix, i)
		assert.Contains(t, services, serviceID, "Should contain service %s", serviceID)
	}

	// Cleanup
	for i := 0; i < 3; i++ {
		client.Agent().ServiceDeregister(fmt.Sprintf("%s-%d", prefix, i))
	}
}

// ==================== P1: Important Tests ====================

// CA-004: Test get service detail
func TestAgentServiceDetail(t *testing.T) {
	client := getClient(t)
	serviceID := "ca004-detail-" + randomID()

	// Register with full details
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:      serviceID,
		Name:    "detail-service",
		Port:    9090,
		Address: "10.0.0.1",
		Tags:    []string{"primary", "v2"},
		Meta: map[string]string{
			"env":     "production",
			"version": "2.0.0",
		},
	})
	require.NoError(t, err)
	time.Sleep(500 * time.Millisecond)

	// Get service detail
	service, _, err := client.Agent().Service(serviceID, nil)
	assert.NoError(t, err)
	assert.NotNil(t, service)
	assert.Equal(t, "detail-service", service.Service)
	assert.Equal(t, 9090, service.Port)
	assert.Equal(t, "10.0.0.1", service.Address)

	// Cleanup
	client.Agent().ServiceDeregister(serviceID)
}

// CA-005: Test register health check
func TestAgentCheckRegister(t *testing.T) {
	client := getClient(t)
	serviceID := "ca005-check-" + randomID()
	checkID := "check-" + serviceID

	// Register service first
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceID,
		Name: "check-service",
		Port: 8080,
	})
	require.NoError(t, err)

	// Register TTL check
	err = client.Agent().CheckRegister(&api.AgentCheckRegistration{
		ID:        checkID,
		Name:      "service-ttl-check",
		ServiceID: serviceID,
		AgentServiceCheck: api.AgentServiceCheck{
			TTL: "30s",
		},
	})
	assert.NoError(t, err, "Check registration should succeed")

	time.Sleep(500 * time.Millisecond)

	// Verify check exists
	checks, err := client.Agent().Checks()
	assert.NoError(t, err)
	assert.Contains(t, checks, checkID)

	// Cleanup
	client.Agent().CheckDeregister(checkID)
	client.Agent().ServiceDeregister(serviceID)
}

// CA-006: Test deregister health check
func TestAgentCheckDeregister(t *testing.T) {
	client := getClient(t)
	checkID := "ca006-check-" + randomID()

	// Register check
	err := client.Agent().CheckRegister(&api.AgentCheckRegistration{
		ID:   checkID,
		Name: "temp-check",
		AgentServiceCheck: api.AgentServiceCheck{
			TTL: "30s",
		},
	})
	require.NoError(t, err)
	time.Sleep(500 * time.Millisecond)

	// Deregister
	err = client.Agent().CheckDeregister(checkID)
	assert.NoError(t, err)

	time.Sleep(500 * time.Millisecond)

	// Verify deregistered
	checks, err := client.Agent().Checks()
	assert.NoError(t, err)
	assert.NotContains(t, checks, checkID)
}

// ==================== P2: Nice to Have Tests ====================

// CA-007: Test pass TTL check
func TestAgentPassTTL(t *testing.T) {
	client := getClient(t)
	checkID := "ca007-ttl-" + randomID()

	// Register TTL check
	err := client.Agent().CheckRegister(&api.AgentCheckRegistration{
		ID:   checkID,
		Name: "ttl-pass-check",
		AgentServiceCheck: api.AgentServiceCheck{
			TTL: "30s",
		},
	})
	require.NoError(t, err)
	time.Sleep(500 * time.Millisecond)

	// Pass TTL
	err = client.Agent().PassTTL(checkID, "All good")
	assert.NoError(t, err, "Pass TTL should succeed")

	// Verify status
	checks, err := client.Agent().Checks()
	assert.NoError(t, err)
	if check, ok := checks[checkID]; ok {
		assert.Equal(t, api.HealthPassing, check.Status)
	}

	// Cleanup
	client.Agent().CheckDeregister(checkID)
}

// CA-008: Test fail TTL check
func TestAgentFailTTL(t *testing.T) {
	client := getClient(t)
	checkID := "ca008-fail-" + randomID()

	// Register TTL check
	err := client.Agent().CheckRegister(&api.AgentCheckRegistration{
		ID:   checkID,
		Name: "ttl-fail-check",
		AgentServiceCheck: api.AgentServiceCheck{
			TTL: "30s",
		},
	})
	require.NoError(t, err)
	time.Sleep(500 * time.Millisecond)

	// Fail TTL
	err = client.Agent().FailTTL(checkID, "Something went wrong")
	assert.NoError(t, err, "Fail TTL should succeed")

	// Verify status
	checks, err := client.Agent().Checks()
	assert.NoError(t, err)
	if check, ok := checks[checkID]; ok {
		assert.Equal(t, api.HealthCritical, check.Status)
	}

	// Cleanup
	client.Agent().CheckDeregister(checkID)
}
