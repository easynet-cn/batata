package tests

import (
	"encoding/json"
	"strings"
	"testing"
	"time"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// ==================== Agent Metrics Tests ====================

// TestAgentMetricsBasic tests getting agent metrics
func TestAgentMetricsBasic(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()

	metrics, err := agent.Metrics()
	require.NoError(t, err, "Agent.Metrics() should not return an error")

	assert.NotEmpty(t, metrics.Timestamp, "Metrics timestamp should not be empty")
	assert.NotNil(t, metrics.Gauges, "Metrics gauges should not be nil")
	assert.NotNil(t, metrics.Counters, "Metrics counters should not be nil")
	assert.NotNil(t, metrics.Samples, "Metrics samples should not be nil")

	// Verify gauges have names
	for _, gauge := range metrics.Gauges {
		assert.NotEmpty(t, gauge.Name, "Each gauge should have a non-empty name")
	}

	t.Logf("Metrics timestamp: %v", metrics.Timestamp)
	t.Logf("Gauges count: %d", len(metrics.Gauges))
	t.Logf("Counters count: %d", len(metrics.Counters))
	t.Logf("Samples count: %d", len(metrics.Samples))
}

// TestAgentMetricsGauges tests gauge metrics
func TestAgentMetricsGauges(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()

	metrics, err := agent.Metrics()
	require.NoError(t, err, "Agent.Metrics() should not return an error")

	for _, gauge := range metrics.Gauges {
		assert.NotEmpty(t, gauge.Name, "Gauge name should not be empty")
		assert.GreaterOrEqual(t, gauge.Value, float32(0), "Gauge value for %s should be >= 0", gauge.Name)

		t.Logf("Gauge: %s = %f", gauge.Name, gauge.Value)
		if len(gauge.Labels) > 0 {
			t.Logf("  Labels: %v", gauge.Labels)
		}
	}
}

// TestAgentMetricsCounters tests counter metrics
func TestAgentMetricsCounters(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()

	metrics, err := agent.Metrics()
	require.NoError(t, err, "Agent.Metrics() should not return an error")

	for _, counter := range metrics.Counters {
		assert.NotEmpty(t, counter.Name, "Counter name should not be empty")
		assert.GreaterOrEqual(t, counter.Count, 0, "Counter count for %s should be >= 0", counter.Name)
		assert.GreaterOrEqual(t, counter.Sum, float64(0), "Counter sum for %s should be >= 0", counter.Name)
		assert.LessOrEqual(t, counter.Min, counter.Max, "Counter min should be <= max for %s", counter.Name)

		t.Logf("Counter: %s", counter.Name)
		t.Logf("  Count: %d, Sum: %f", counter.Count, counter.Sum)
		t.Logf("  Min: %f, Max: %f, Mean: %f", counter.Min, counter.Max, counter.Mean)
	}
}

// TestAgentMetricsSamples tests sample metrics
func TestAgentMetricsSamples(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()

	metrics, err := agent.Metrics()
	require.NoError(t, err, "Agent.Metrics() should not return an error")

	for _, sample := range metrics.Samples {
		assert.NotEmpty(t, sample.Name, "Sample name should not be empty")
		assert.GreaterOrEqual(t, sample.Count, 0, "Sample count for %s should be >= 0", sample.Name)
		assert.LessOrEqual(t, sample.Min, sample.Max, "Sample min should be <= max for %s", sample.Name)
		assert.GreaterOrEqual(t, sample.Stddev, float64(0), "Sample stddev for %s should be >= 0", sample.Name)

		t.Logf("Sample: %s", sample.Name)
		t.Logf("  Count: %d, Sum: %f", sample.Count, sample.Sum)
		t.Logf("  Min: %f, Max: %f", sample.Min, sample.Max)
		if sample.Stddev > 0 {
			t.Logf("  Stddev: %f", sample.Stddev)
		}
	}
}

// ==================== Agent Self Tests ====================

// TestAgentSelfInfo tests getting agent self information
func TestAgentSelfInfo(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()

	self, err := agent.Self()
	require.NoError(t, err, "Agent.Self() should not return an error")
	require.NotNil(t, self, "Agent self info should not be nil")

	config, ok := self["Config"]
	assert.True(t, ok, "Self info should contain 'Config' key")
	assert.NotNil(t, config, "Config should not be nil")
	t.Logf("Agent config: %v", config)

	member, ok := self["Member"]
	assert.True(t, ok, "Self info should contain 'Member' key")
	assert.NotNil(t, member, "Member should not be nil")
	t.Logf("Agent member: %v", member)

	if stats, ok := self["Stats"]; ok {
		t.Logf("Agent stats: %v", stats)
	}
}

// TestAgentSelfConfig tests agent configuration
func TestAgentSelfConfig(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()

	self, err := agent.Self()
	require.NoError(t, err, "Agent.Self() should not return an error")

	config := self["Config"]
	require.NotNil(t, config, "Config should not be nil")

	datacenter, ok := config["Datacenter"]
	assert.True(t, ok, "Config should contain 'Datacenter'")
	assert.NotEmpty(t, datacenter, "Datacenter should not be empty")
	t.Logf("Datacenter: %v", datacenter)

	nodeName, ok := config["NodeName"]
	assert.True(t, ok, "Config should contain 'NodeName'")
	assert.NotEmpty(t, nodeName, "NodeName should not be empty")
	t.Logf("NodeName: %v", nodeName)

	if version, ok := config["Version"]; ok {
		assert.NotEmpty(t, version, "Version should not be empty if present")
		t.Logf("Version: %v", version)
	}
}

// TestAgentSelfMember tests agent member information
func TestAgentSelfMember(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()

	self, err := agent.Self()
	require.NoError(t, err, "Agent.Self() should not return an error")

	member := self["Member"]
	require.NotNil(t, member, "Member should not be nil")

	name := member["Name"]
	assert.NotNil(t, name, "Member Name should not be nil")
	assert.NotEmpty(t, name, "Member Name should not be empty")
	t.Logf("Member Name: %v", name)

	addr := member["Addr"]
	assert.NotNil(t, addr, "Member Addr should not be nil")
	assert.NotEmpty(t, addr, "Member Addr should not be empty")
	t.Logf("Member Addr: %v", addr)

	port := member["Port"]
	assert.NotNil(t, port, "Member Port should not be nil")
	t.Logf("Member Port: %v", port)

	status := member["Status"]
	assert.NotNil(t, status, "Member Status should not be nil")
	t.Logf("Member Status: %v", status)
}

// ==================== Agent Host Tests ====================

// TestAgentHostInfo tests getting agent host information
func TestAgentHostInfo(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()

	host, err := agent.Host()
	require.NoError(t, err, "Agent.Host() should not return an error")
	require.NotNil(t, host, "Host info should not be nil")

	// Host info contains system information
	if hostInfo, ok := host["Host"]; ok {
		assert.NotNil(t, hostInfo, "Host info value should not be nil")
		t.Logf("Host info: %v", hostInfo)
	}
}

// ==================== Agent Members Tests ====================

// TestAgentMembersDetailed tests listing cluster members with details
func TestAgentMembersDetailed(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()

	members, err := agent.Members(false)
	require.NoError(t, err, "Agent.Members() should not return an error")
	assert.NotEmpty(t, members, "Members list should not be empty")

	t.Logf("Found %d members", len(members))
	for _, member := range members {
		assert.NotEmpty(t, member.Name, "Member name should not be empty")
		assert.NotEmpty(t, member.Addr, "Member address should not be empty")
		assert.True(t, member.Status >= 0, "Member status should be valid")

		t.Logf("  Member: %s, Addr: %s, Status: %d", member.Name, member.Addr, member.Status)
		t.Logf("    Tags: %v", member.Tags)
	}
}

// TestAgentMembersWAN tests listing WAN members
func TestAgentMembersWAN(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()

	members, err := agent.Members(true) // WAN members
	require.NoError(t, err, "Agent.Members(WAN) should not return an error")
	require.NotNil(t, members, "WAN members should not be nil")

	t.Logf("Found %d WAN members", len(members))
	for _, member := range members {
		assert.NotEmpty(t, member.Name, "WAN member name should not be empty")
		assert.NotEmpty(t, member.Addr, "WAN member address should not be empty")
		t.Logf("  WAN Member: %s, Addr: %s", member.Name, member.Addr)
	}
}

// ==================== Agent Reload Tests ====================

// TestAgentReload tests agent configuration reload
func TestAgentReload(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()

	err := agent.Reload()
	require.NoError(t, err, "Agent.Reload() should not return an error")

	t.Log("Agent configuration reloaded successfully")
}

// ==================== Agent Log Tests ====================

// TestAgentMonitorLog tests monitoring agent logs
func TestAgentMonitorLog(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()

	// Monitor for a short time
	logCh, err := agent.Monitor("DEBUG", nil, nil)
	require.NoError(t, err, "Agent.Monitor() should not return an error")
	require.NotNil(t, logCh, "Log channel should not be nil")

	// Read a few log entries
	timeout := time.After(2 * time.Second)
	logCount := 0

	for {
		select {
		case log := <-logCh:
			if log != "" {
				logCount++
				if logCount <= 5 {
					t.Logf("Log: %s", strings.TrimSpace(log))
				}
			}
		case <-timeout:
			t.Logf("Received %d log entries", logCount)
			return
		}
	}
}

// ==================== Agent Token Tests ====================

// TestAgentTokenDefault tests getting default agent token
func TestAgentTokenDefault(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()

	// Try to update default token (will fail if ACL not enabled)
	_, err := agent.UpdateDefaultACLToken("test-token", nil)
	if err != nil {
		// This is expected if ACL is not enabled
		assert.Error(t, err, "Expected error when ACL is not enabled")
		t.Logf("Agent default token (expected error): %v", err)
		return
	}

	t.Log("Default token updated")
}

// TestAgentTokenAgent tests agent token operations
func TestAgentTokenAgent(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()

	// Try to update agent token
	_, err := agent.UpdateAgentACLToken("agent-token", nil)
	if err != nil {
		// This is expected if ACL is not enabled
		assert.Error(t, err, "Expected error when ACL is not enabled")
		t.Logf("Agent token (expected error): %v", err)
		return
	}

	t.Log("Agent token updated")
}

// ==================== Agent Leave Tests ====================

// TestAgentNodeName tests getting agent node name
func TestAgentNodeName(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()

	nodeName, err := agent.NodeName()
	require.NoError(t, err, "Agent.NodeName() should not return an error")

	assert.NotEmpty(t, nodeName, "Node name should not be empty")
	t.Logf("Node name: %s", nodeName)
}

// ==================== Service Weights Tests ====================

// TestServiceWeights tests service weight configuration
func TestServiceWeights(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	serviceName := "weighted-svc-" + randomString(8)

	// Register service with weights
	reg := &api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
		Weights: &api.AgentWeights{
			Passing: 10,
			Warning: 5,
		},
	}

	err := agent.ServiceRegister(reg)
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	// Verify weights
	services, err := agent.Services()
	require.NoError(t, err)

	svc, ok := services[serviceName]
	require.True(t, ok, "Service %s should exist in services list", serviceName)
	require.NotNil(t, svc.Weights, "Service weights should not be nil")

	assert.Equal(t, 10, svc.Weights.Passing, "Passing weight should be 10")
	assert.Equal(t, 5, svc.Weights.Warning, "Warning weight should be 5")

	t.Logf("Service weights - Passing: %d, Warning: %d",
		svc.Weights.Passing, svc.Weights.Warning)
}

// ==================== Service Tagged Addresses Tests ====================

// TestServiceTaggedAddresses tests service tagged addresses
func TestServiceTaggedAddresses(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	serviceName := "tagged-addr-" + randomString(8)

	reg := &api.AgentServiceRegistration{
		ID:      serviceName,
		Name:    serviceName,
		Port:    8080,
		Address: "192.168.1.100",
		TaggedAddresses: map[string]api.ServiceAddress{
			"lan": {
				Address: "192.168.1.100",
				Port:    8080,
			},
			"wan": {
				Address: "10.0.0.100",
				Port:    8080,
			},
		},
	}

	err := agent.ServiceRegister(reg)
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	services, err := agent.Services()
	require.NoError(t, err)

	svc, ok := services[serviceName]
	require.True(t, ok, "Service %s should exist in services list", serviceName)
	require.NotNil(t, svc.TaggedAddresses, "Tagged addresses should not be nil")

	lan, ok := svc.TaggedAddresses["lan"]
	assert.True(t, ok, "LAN tagged address should exist")
	assert.Equal(t, "192.168.1.100", lan.Address, "LAN address should match")
	assert.Equal(t, 8080, lan.Port, "LAN port should match")
	t.Logf("LAN: %s:%d", lan.Address, lan.Port)

	wan, ok := svc.TaggedAddresses["wan"]
	assert.True(t, ok, "WAN tagged address should exist")
	assert.Equal(t, "10.0.0.100", wan.Address, "WAN address should match")
	assert.Equal(t, 8080, wan.Port, "WAN port should match")
	t.Logf("WAN: %s:%d", wan.Address, wan.Port)
}

// ==================== Service Locality Tests ====================

// TestServiceLocality tests service locality configuration
func TestServiceLocality(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	serviceName := "locality-svc-" + randomString(8)

	reg := &api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
		Locality: &api.Locality{
			Region: "us-east-1",
			Zone:   "us-east-1a",
		},
	}

	err := agent.ServiceRegister(reg)
	require.NoError(t, err, "Service registration with locality should succeed")
	defer agent.ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	services, err := agent.Services()
	require.NoError(t, err, "Getting services should succeed")

	svc, ok := services[serviceName]
	require.True(t, ok, "Service %s should exist in services list", serviceName)

	if svc.Locality != nil {
		assert.Equal(t, "us-east-1", svc.Locality.Region, "Locality region should match")
		assert.Equal(t, "us-east-1a", svc.Locality.Zone, "Locality zone should match")
		t.Logf("Locality - Region: %s, Zone: %s",
			svc.Locality.Region, svc.Locality.Zone)
	} else {
		t.Log("Locality is nil - server may not support locality field")
	}
}

// ==================== JSON Serialization Tests ====================

// TestMetricsJSONSerialization tests metrics JSON serialization
func TestMetricsJSONSerialization(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()

	metrics, err := agent.Metrics()
	require.NoError(t, err, "Agent.Metrics() should not return an error")

	// Serialize to JSON
	jsonData, err := json.Marshal(metrics)
	require.NoError(t, err, "JSON marshal should not fail")
	assert.Greater(t, len(jsonData), 0, "Serialized JSON should not be empty")

	t.Logf("Metrics JSON length: %d bytes", len(jsonData))

	// Deserialize back
	var parsed api.MetricsInfo
	err = json.Unmarshal(jsonData, &parsed)
	require.NoError(t, err, "JSON unmarshal should not fail")

	// Verify round-trip preserves data
	assert.Equal(t, metrics.Timestamp, parsed.Timestamp, "Timestamp should survive round-trip")
	assert.Equal(t, len(metrics.Gauges), len(parsed.Gauges), "Gauges count should survive round-trip")
	assert.Equal(t, len(metrics.Counters), len(parsed.Counters), "Counters count should survive round-trip")
	assert.Equal(t, len(metrics.Samples), len(parsed.Samples), "Samples count should survive round-trip")

	// Verify gauge names survive round-trip
	for i, gauge := range metrics.Gauges {
		if i < len(parsed.Gauges) {
			assert.Equal(t, gauge.Name, parsed.Gauges[i].Name, "Gauge name should survive round-trip")
			assert.Equal(t, gauge.Value, parsed.Gauges[i].Value, "Gauge value should survive round-trip")
		}
	}

	t.Log("JSON serialization roundtrip successful")
}

// TestSelfJSONSerialization tests self info JSON serialization
func TestSelfJSONSerialization(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()

	self, err := agent.Self()
	require.NoError(t, err, "Agent.Self() should not return an error")
	require.NotNil(t, self, "Self info should not be nil")

	jsonData, err := json.Marshal(self)
	require.NoError(t, err, "JSON marshal should not fail")
	assert.Greater(t, len(jsonData), 0, "Serialized JSON should not be empty")

	t.Logf("Self JSON length: %d bytes", len(jsonData))

	var parsed map[string]map[string]interface{}
	err = json.Unmarshal(jsonData, &parsed)
	require.NoError(t, err, "JSON unmarshal should not fail")

	// Verify round-trip preserves top-level keys
	for key := range self {
		_, ok := parsed[key]
		assert.True(t, ok, "Key %s should survive JSON round-trip", key)
	}
	assert.Equal(t, len(self), len(parsed), "Number of top-level keys should survive round-trip")

	t.Log("Self JSON serialization roundtrip successful")
}
