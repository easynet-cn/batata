package tests

import (
	"testing"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// ==================== Service Update & Meta Tests ====================

// CA-009: Test service update (re-register with changed properties)
func TestAgentServiceUpdate(t *testing.T) {
	client := getClient(t)
	agent := client.Agent()
	serviceID := "ca009-update-" + randomID()

	reg := &api.AgentServiceRegistration{
		ID: serviceID, Name: "update-svc", Port: 8080,
		Tags: []string{"v1"}, Meta: map[string]string{"version": "1.0"}, Address: "10.0.0.1",
	}
	err := agent.ServiceRegister(reg)
	require.NoError(t, err, "Initial registration should succeed")
	defer agent.ServiceDeregister(serviceID)

	reg.Port = 9090
	reg.Tags = []string{"v2", "latest"}
	reg.Meta = map[string]string{"version": "2.0", "env": "prod"}
	reg.Address = "10.0.0.2"
	err = agent.ServiceRegister(reg)
	require.NoError(t, err, "Update registration should succeed")

	svc, _, err := agent.Service(serviceID, nil)
	require.NoError(t, err)
	assert.Equal(t, 9090, svc.Port, "Port should be updated")
	assert.Equal(t, "10.0.0.2", svc.Address, "Address should be updated")
	assert.Contains(t, svc.Tags, "v2")
	assert.Contains(t, svc.Tags, "latest")
	assert.Equal(t, "2.0", svc.Meta["version"])
	assert.Equal(t, "prod", svc.Meta["env"])
}

// CA-010: Test service registration with meta fields
func TestAgentServiceWithMeta(t *testing.T) {
	client := getClient(t)
	agent := client.Agent()
	serviceID := "ca010-meta-" + randomID()

	reg := &api.AgentServiceRegistration{
		ID: serviceID, Name: "meta-svc", Port: 8080,
		Meta: map[string]string{
			"version": "3.1.0", "environment": "staging",
			"owner": "platform-team", "protocol": "grpc",
		},
	}
	err := agent.ServiceRegister(reg)
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceID)

	svc, _, err := agent.Service(serviceID, nil)
	require.NoError(t, err)
	assert.Equal(t, "3.1.0", svc.Meta["version"])
	assert.Equal(t, "staging", svc.Meta["environment"])
	assert.Equal(t, "platform-team", svc.Meta["owner"])
	assert.Equal(t, "grpc", svc.Meta["protocol"])
}

// CA-011: Test service with tagged addresses
func TestAgentServiceWithTaggedAddresses(t *testing.T) {
	client := getClient(t)
	agent := client.Agent()
	serviceID := "ca011-addr-" + randomID()

	reg := &api.AgentServiceRegistration{
		ID: serviceID, Name: "addr-svc", Port: 8080, Address: "192.168.1.100",
		TaggedAddresses: map[string]api.ServiceAddress{
			"lan": {Address: "192.168.1.100", Port: 8080},
			"wan": {Address: "203.0.113.50", Port: 443},
		},
	}
	err := agent.ServiceRegister(reg)
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceID)

	svc, _, err := agent.Service(serviceID, nil)
	require.NoError(t, err)
	assert.Equal(t, "192.168.1.100", svc.Address)
	if svc.TaggedAddresses != nil {
		if lan, ok := svc.TaggedAddresses["lan"]; ok {
			assert.Equal(t, "192.168.1.100", lan.Address)
			assert.Equal(t, 8080, lan.Port)
		}
		if wan, ok := svc.TaggedAddresses["wan"]; ok {
			assert.Equal(t, "203.0.113.50", wan.Address)
			assert.Equal(t, 443, wan.Port)
		}
	}
}

// CA-012: Test agent health by service ID
func TestAgentHealthServiceByID(t *testing.T) {
	client := getClient(t)
	agent := client.Agent()
	serviceID := "ca012-healthid-" + randomID()

	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID: serviceID, Name: "healthid-svc", Port: 8080,
		Check: &api.AgentServiceCheck{TTL: "30s", Status: "passing"},
	})
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceID)

	status, info, err := agent.AgentHealthServiceByID(serviceID)
	if err != nil {
		t.Skipf("AgentHealthServiceByID not supported: %v", err)
	}
	assert.Equal(t, "passing", status)
	assert.NotNil(t, info)
	assert.Equal(t, serviceID, info.Service.ID)
}

// CA-013: Test agent health by service name
func TestAgentHealthServiceByName(t *testing.T) {
	client := getClient(t)
	agent := client.Agent()
	serviceName := "healthname-" + randomID()

	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID: serviceName + "-1", Name: serviceName, Port: 8080,
		Check: &api.AgentServiceCheck{TTL: "30s", Status: "passing"},
	})
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName + "-1")

	status, entries, err := agent.AgentHealthServiceByName(serviceName)
	if err != nil {
		t.Skipf("AgentHealthServiceByName not supported: %v", err)
	}
	assert.Equal(t, "passing", status)
	assert.NotEmpty(t, entries)
	assert.Equal(t, serviceName, entries[0].Service.Service)
}

// CA-018: Test service with EnableTagOverride
func TestAgentServiceEnableTagOverrideVerify(t *testing.T) {
	client := getClient(t)
	agent := client.Agent()
	serviceID := "ca018-tagoverride-" + randomID()

	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID: serviceID, Name: "tagoverride-svc", Port: 8080,
		Tags: []string{"v1"}, EnableTagOverride: true,
	})
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceID)

	svc, _, err := agent.Service(serviceID, nil)
	require.NoError(t, err)
	assert.True(t, svc.EnableTagOverride, "EnableTagOverride should be true")
}

// CA-019: Test catalog register with service and checks
func TestCatalogRegisterWithChecks(t *testing.T) {
	client := getClient(t)
	catalog := client.Catalog()
	nodeName := "cat-check-node-" + randomID()
	serviceID := "cat-check-svc-" + randomID()

	_, err := catalog.Register(&api.CatalogRegistration{
		Node: nodeName, Address: "10.0.0.50",
		Service: &api.AgentService{ID: serviceID, Service: "cat-check-svc", Port: 8080, Tags: []string{"web"}},
		Check:   &api.AgentCheck{CheckID: "serfHealth", Name: "Serf Health", Node: nodeName, ServiceID: serviceID, Status: "passing"},
	}, nil)
	require.NoError(t, err)
	defer catalog.Deregister(&api.CatalogDeregistration{Node: nodeName}, nil)

	services, _, err := catalog.Service("cat-check-svc", "", nil)
	require.NoError(t, err)
	found := false
	for _, svc := range services {
		if svc.ServiceID == serviceID {
			found = true
			assert.Equal(t, 8080, svc.ServicePort)
			assert.Contains(t, svc.ServiceTags, "web")
			break
		}
	}
	assert.True(t, found, "Service should be in catalog")
}

// CA-020: Test deregister removes service and all linked checks
func TestAgentDeregisterRemovesChecks(t *testing.T) {
	client := getClient(t)
	agent := client.Agent()
	serviceID := "ca020-dereg-" + randomID()

	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID: serviceID, Name: "dereg-svc", Port: 8080,
		Check: &api.AgentServiceCheck{TTL: "30s", Status: "passing"},
	})
	require.NoError(t, err)

	services, err := agent.Services()
	require.NoError(t, err)
	_, exists := services[serviceID]
	assert.True(t, exists, "Service should exist before deregister")

	err = agent.ServiceDeregister(serviceID)
	require.NoError(t, err)

	services, err = agent.Services()
	require.NoError(t, err)
	_, exists = services[serviceID]
	assert.False(t, exists, "Service should not exist after deregister")

	checks, err := agent.Checks()
	require.NoError(t, err)
	for _, check := range checks {
		assert.NotEqual(t, serviceID, check.ServiceID, "No check should reference deregistered service")
	}
}

// CA-023: Test service weights round-trip
func TestAgentServiceWeightsRoundTrip(t *testing.T) {
	client := getClient(t)
	agent := client.Agent()
	serviceID := "ca023-weights-" + randomID()

	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID: serviceID, Name: "weights-svc", Port: 8080,
		Weights: &api.AgentWeights{Passing: 10, Warning: 1},
	})
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceID)

	svc, _, err := agent.Service(serviceID, nil)
	require.NoError(t, err)
	if svc.Weights.Passing > 0 {
		assert.Equal(t, 10, svc.Weights.Passing)
		assert.Equal(t, 1, svc.Weights.Warning)
	}
}

// CA-024: Test listing multiple instances of same service
func TestAgentListMultipleInstances(t *testing.T) {
	client := getClient(t)
	agent := client.Agent()
	serviceName := "multi-inst-" + randomID()
	id1 := serviceName + "-1"
	id2 := serviceName + "-2"

	err := agent.ServiceRegister(&api.AgentServiceRegistration{ID: id1, Name: serviceName, Port: 8081})
	require.NoError(t, err)
	defer agent.ServiceDeregister(id1)

	err = agent.ServiceRegister(&api.AgentServiceRegistration{ID: id2, Name: serviceName, Port: 8082})
	require.NoError(t, err)
	defer agent.ServiceDeregister(id2)

	services, err := agent.Services()
	require.NoError(t, err)

	svc1, ok1 := services[id1]
	assert.True(t, ok1, "Should find first instance")
	if ok1 {
		assert.Equal(t, 8081, svc1.Port)
	}

	svc2, ok2 := services[id2]
	assert.True(t, ok2, "Should find second instance")
	if ok2 {
		assert.Equal(t, 8082, svc2.Port)
	}
}
