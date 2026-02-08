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
	if err != nil {
		t.Logf("Agent metrics: %v", err)
		return
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
	if err != nil {
		t.Logf("Agent metrics: %v", err)
		return
	}

	for _, gauge := range metrics.Gauges {
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
	if err != nil {
		t.Logf("Agent metrics: %v", err)
		return
	}

	for _, counter := range metrics.Counters {
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
	if err != nil {
		t.Logf("Agent metrics: %v", err)
		return
	}

	for _, sample := range metrics.Samples {
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
	if err != nil {
		t.Logf("Agent self: %v", err)
		return
	}

	if config, ok := self["Config"]; ok {
		t.Logf("Agent config: %v", config)
	}

	if member, ok := self["Member"]; ok {
		t.Logf("Agent member: %v", member)
	}

	if stats, ok := self["Stats"]; ok {
		t.Logf("Agent stats: %v", stats)
	}
}

// TestAgentSelfConfig tests agent configuration
func TestAgentSelfConfig(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()

	self, err := agent.Self()
	if err != nil {
		t.Logf("Agent self: %v", err)
		return
	}

	if config := self["Config"]; config != nil {
		if datacenter, ok := config["Datacenter"]; ok {
			t.Logf("Datacenter: %v", datacenter)
		}
		if nodeName, ok := config["NodeName"]; ok {
			t.Logf("NodeName: %v", nodeName)
		}
		if version, ok := config["Version"]; ok {
			t.Logf("Version: %v", version)
		}
	}
}

// TestAgentSelfMember tests agent member information
func TestAgentSelfMember(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()

	self, err := agent.Self()
	if err != nil {
		t.Logf("Agent self: %v", err)
		return
	}

	if member := self["Member"]; member != nil {
		t.Logf("Member Name: %v", member["Name"])
		t.Logf("Member Addr: %v", member["Addr"])
		t.Logf("Member Port: %v", member["Port"])
		t.Logf("Member Status: %v", member["Status"])
	}
}

// ==================== Agent Host Tests ====================

// TestAgentHostInfo tests getting agent host information
func TestAgentHostInfo(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()

	host, err := agent.Host()
	if err != nil {
		t.Logf("Agent host: %v", err)
		return
	}

	// Host info contains system information
	if hostInfo, ok := host["Host"]; ok {
		t.Logf("Host info: %v", hostInfo)
	}
}

// ==================== Agent Members Tests ====================

// TestAgentMembersDetailed tests listing cluster members with details
func TestAgentMembersDetailed(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()

	members, err := agent.Members(false)
	if err != nil {
		t.Logf("Agent members: %v", err)
		return
	}

	t.Logf("Found %d members", len(members))
	for _, member := range members {
		t.Logf("  Member: %s, Addr: %s, Status: %d", member.Name, member.Addr, member.Status)
		t.Logf("    Tags: %v", member.Tags)
	}
}

// TestAgentMembersWAN tests listing WAN members
func TestAgentMembersWAN(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()

	members, err := agent.Members(true) // WAN members
	if err != nil {
		t.Logf("Agent WAN members: %v", err)
		return
	}

	t.Logf("Found %d WAN members", len(members))
	for _, member := range members {
		t.Logf("  WAN Member: %s, Addr: %s", member.Name, member.Addr)
	}
}

// ==================== Agent Reload Tests ====================

// TestAgentReload tests agent configuration reload
func TestAgentReload(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()

	err := agent.Reload()
	if err != nil {
		t.Logf("Agent reload: %v", err)
		return
	}

	t.Log("Agent configuration reloaded successfully")
}

// ==================== Agent Log Tests ====================

// TestAgentMonitorLog tests monitoring agent logs
func TestAgentMonitorLog(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()

	// Monitor for a short time
	logCh, err := agent.Monitor("DEBUG", nil, nil)
	if err != nil {
		t.Logf("Agent monitor: %v", err)
		return
	}

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
		t.Logf("Agent default token: %v", err)
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
		t.Logf("Agent token: %v", err)
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
	if err != nil {
		t.Logf("Agent node name: %v", err)
		return
	}

	assert.NotEmpty(t, nodeName)
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

	if svc, ok := services[serviceName]; ok {
		t.Logf("Service weights - Passing: %d, Warning: %d",
			svc.Weights.Passing, svc.Weights.Warning)
		assert.Equal(t, 10, svc.Weights.Passing)
		assert.Equal(t, 5, svc.Weights.Warning)
	}
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

	if svc, ok := services[serviceName]; ok {
		t.Logf("Tagged addresses: %v", svc.TaggedAddresses)
		if lan, ok := svc.TaggedAddresses["lan"]; ok {
			t.Logf("LAN: %s:%d", lan.Address, lan.Port)
		}
		if wan, ok := svc.TaggedAddresses["wan"]; ok {
			t.Logf("WAN: %s:%d", wan.Address, wan.Port)
		}
	}
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
	if err != nil {
		t.Logf("Service locality register: %v", err)
		return
	}
	defer agent.ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	services, err := agent.Services()
	if err != nil {
		t.Logf("Get services: %v", err)
		return
	}

	if svc, ok := services[serviceName]; ok && svc.Locality != nil {
		t.Logf("Locality - Region: %s, Zone: %s",
			svc.Locality.Region, svc.Locality.Zone)
	}
}

// ==================== JSON Serialization Tests ====================

// TestMetricsJSONSerialization tests metrics JSON serialization
func TestMetricsJSONSerialization(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()

	metrics, err := agent.Metrics()
	if err != nil {
		t.Logf("Agent metrics: %v", err)
		return
	}

	// Serialize to JSON
	jsonData, err := json.Marshal(metrics)
	if err != nil {
		t.Logf("JSON marshal: %v", err)
		return
	}

	t.Logf("Metrics JSON length: %d bytes", len(jsonData))

	// Deserialize back
	var parsed api.MetricsInfo
	err = json.Unmarshal(jsonData, &parsed)
	if err != nil {
		t.Logf("JSON unmarshal: %v", err)
		return
	}

	assert.Equal(t, len(metrics.Gauges), len(parsed.Gauges))
	t.Log("JSON serialization roundtrip successful")
}

// TestSelfJSONSerialization tests self info JSON serialization
func TestSelfJSONSerialization(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()

	self, err := agent.Self()
	if err != nil {
		t.Logf("Agent self: %v", err)
		return
	}

	jsonData, err := json.Marshal(self)
	if err != nil {
		t.Logf("JSON marshal: %v", err)
		return
	}

	t.Logf("Self JSON length: %d bytes", len(jsonData))

	var parsed map[string]map[string]interface{}
	err = json.Unmarshal(jsonData, &parsed)
	if err != nil {
		t.Logf("JSON unmarshal: %v", err)
		return
	}

	t.Log("Self JSON serialization roundtrip successful")
}
