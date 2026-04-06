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

// CA-009: Test Agent Self response structure validation
func TestAgentSelfStructure(t *testing.T) {
	client := getClient(t)

	self, err := client.Agent().Self()
	require.NoError(t, err, "Agent self should succeed")
	require.NotNil(t, self)

	// Config section must exist with required fields
	config, ok := self["Config"]
	require.True(t, ok, "Self must have 'Config' section")
	require.NotNil(t, config)

	// Required Config fields (Consul contract)
	nodeName, ok := config["NodeName"]
	assert.True(t, ok, "Config must have 'NodeName'")
	assert.NotEmpty(t, nodeName, "NodeName must not be empty")

	datacenter, ok := config["Datacenter"]
	assert.True(t, ok, "Config must have 'Datacenter'")
	assert.NotEmpty(t, datacenter, "Datacenter must not be empty")

	nodeID, ok := config["NodeID"]
	assert.True(t, ok, "Config must have 'NodeID'")
	assert.NotEmpty(t, nodeID, "NodeID must not be empty")

	version, ok := config["Version"]
	assert.True(t, ok, "Config must have 'Version'")
	assert.NotEmpty(t, version, "Version must not be empty")

	server, ok := config["Server"]
	assert.True(t, ok, "Config must have 'Server'")
	assert.Equal(t, true, server, "Server must be true")

	// Member section must exist
	member, ok := self["Member"]
	assert.True(t, ok, "Self must have 'Member' section")
	assert.NotNil(t, member, "Member must not be nil")

	// Stats section should exist
	_, ok = self["Stats"]
	assert.True(t, ok, "Self must have 'Stats' section")

	// Meta section should exist
	_, ok = self["Meta"]
	assert.True(t, ok, "Self must have 'Meta' section")
}

// CA-010: Test Agent Members returns valid member list
func TestAgentMembersValidation(t *testing.T) {
	client := getClient(t)

	members, err := client.Agent().Members(false)
	require.NoError(t, err, "Agent members should succeed")
	require.NotEmpty(t, members, "Must have at least one member")

	// Validate first member
	member := members[0]
	assert.NotEmpty(t, member.Name, "Member.Name must not be empty")
	assert.NotEmpty(t, member.Addr, "Member.Addr must not be empty")
	assert.Greater(t, member.Port, uint16(0), "Member.Port must be > 0")
	// Status 1 = Alive (serf.StatusAlive)
	assert.Equal(t, int(1), int(member.Status),
		"Member.Status must be 1 (Alive)")
	assert.NotNil(t, member.Tags, "Member.Tags must not be nil")
}

// CA-011: Test Agent service registration with full field round-trip
func TestAgentServiceRoundTrip(t *testing.T) {
	client := getClient(t)
	serviceID := "ca011-roundtrip-" + randomID()

	reg := &api.AgentServiceRegistration{
		ID:      serviceID,
		Name:    "roundtrip-service",
		Port:    9090,
		Address: "10.0.0.1",
		Tags:    []string{"primary", "v2", "prod"},
		Meta: map[string]string{
			"env":     "production",
			"version": "2.0.0",
			"region":  "us-east-1",
		},
	}

	err := client.Agent().ServiceRegister(reg)
	require.NoError(t, err)
	time.Sleep(500 * time.Millisecond)
	defer client.Agent().ServiceDeregister(serviceID)

	// Verify via Agent().Services() (map of all services)
	services, err := client.Agent().Services()
	require.NoError(t, err)

	svc, exists := services[serviceID]
	require.True(t, exists, "Service must exist in services map")

	// Full field validation
	assert.Equal(t, serviceID, svc.ID, "ID must match")
	assert.Equal(t, "roundtrip-service", svc.Service, "Service name must match")
	assert.Equal(t, 9090, svc.Port, "Port must match")
	assert.Equal(t, "10.0.0.1", svc.Address, "Address must match")

	// Tags — all present
	require.Len(t, svc.Tags, 3, "Must have 3 tags")
	assert.Contains(t, svc.Tags, "primary")
	assert.Contains(t, svc.Tags, "v2")
	assert.Contains(t, svc.Tags, "prod")

	// Meta — all present
	require.Len(t, svc.Meta, 3, "Must have 3 meta entries")
	assert.Equal(t, "production", svc.Meta["env"])
	assert.Equal(t, "2.0.0", svc.Meta["version"])
	assert.Equal(t, "us-east-1", svc.Meta["region"])
}

// CA-012: Test TTL check status transitions with output validation
func TestAgentCheckStatusTransitionsStrict(t *testing.T) {
	client := getClient(t)
	checkID := "ca012-transitions-" + randomID()

	err := client.Agent().CheckRegister(&api.AgentCheckRegistration{
		ID:   checkID,
		Name: "transition-check",
		AgentServiceCheck: api.AgentServiceCheck{
			TTL: "60s",
		},
	})
	require.NoError(t, err)
	defer client.Agent().CheckDeregister(checkID)
	time.Sleep(500 * time.Millisecond)

	// Pass
	err = client.Agent().PassTTL(checkID, "healthy")
	require.NoError(t, err)
	checks, _ := client.Agent().Checks()
	require.Contains(t, checks, checkID)
	assert.Equal(t, api.HealthPassing, checks[checkID].Status,
		"Status must be 'passing' after PassTTL")
	assert.Equal(t, "healthy", checks[checkID].Output,
		"Output must match PassTTL note")

	// Warn
	err = client.Agent().WarnTTL(checkID, "degraded")
	require.NoError(t, err)
	checks, _ = client.Agent().Checks()
	assert.Equal(t, api.HealthWarning, checks[checkID].Status,
		"Status must be 'warning' after WarnTTL")
	assert.Equal(t, "degraded", checks[checkID].Output,
		"Output must match WarnTTL note")

	// Fail
	err = client.Agent().FailTTL(checkID, "down")
	require.NoError(t, err)
	checks, _ = client.Agent().Checks()
	assert.Equal(t, api.HealthCritical, checks[checkID].Status,
		"Status must be 'critical' after FailTTL")
	assert.Equal(t, "down", checks[checkID].Output,
		"Output must match FailTTL note")

	// Back to passing
	err = client.Agent().PassTTL(checkID, "recovered")
	require.NoError(t, err)
	checks, _ = client.Agent().Checks()
	assert.Equal(t, api.HealthPassing, checks[checkID].Status,
		"Status must be 'passing' after recovery")
}

// Tests CA-013+ are in agent_service_test.go, agent_check_test.go, agent_advanced_test.go
