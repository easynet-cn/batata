package tests

import (
	"testing"
	"time"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// ==================== Discovery Chain API Tests ====================

// CDIS-001: Test discovery chain for a non-existent service returns a default chain or empty
func TestDiscoveryChainGetNonExistent(t *testing.T) {
	client := getClient(t)
	serviceName := "cdis-nonexistent-" + randomID()

	// Query the discovery chain for a service that has no config entries
	resp, qm, err := client.DiscoveryChain().Get(serviceName, nil, nil)

	// The discovery chain endpoint compiles chains on demand. For a service
	// with no config entries, Consul returns a default chain (Default=true)
	// rather than an error. We accept either a valid default chain or an error
	// (if the agent doesn't support this endpoint), but NOT a panic.
	if err != nil {
		t.Logf("Discovery chain for non-existent service returned error (acceptable): %v", err)
		return
	}

	require.NotNil(t, resp, "Response should not be nil when no error")
	assert.NotNil(t, qm, "QueryMeta should not be nil")
	assert.Greater(t, qm.LastIndex, uint64(0), "QueryMeta.LastIndex should be > 0")

	if resp.Chain != nil {
		chain := resp.Chain
		assert.Equal(t, serviceName, chain.ServiceName,
			"Chain.ServiceName must match the queried service name")
		// A service with no config entries should produce a default chain
		assert.True(t, chain.Default,
			"Chain.Default should be true for a service with no config entries")
		// Default chain should have at least one node (the resolver)
		if assert.NotEmpty(t, chain.StartNode, "StartNode should not be empty for default chain") {
			assert.Contains(t, chain.Nodes, chain.StartNode,
				"Nodes map should contain the StartNode")
		}
		// Default chain should have at least one target
		assert.NotEmpty(t, chain.Targets,
			"Default chain should have at least one target")
	}
}

// CDIS-002: Test discovery chain after creating a service-resolver config entry
func TestDiscoveryChainWithServiceResolver(t *testing.T) {
	client := getClient(t)
	configEntries := client.ConfigEntries()
	serviceName := "cdis-resolver-" + randomString(8)

	// Create a service-resolver config entry with a default subset
	resolver := &api.ServiceResolverConfigEntry{
		Kind:          api.ServiceResolver,
		Name:          serviceName,
		DefaultSubset: "v1",
		Subsets: map[string]api.ServiceResolverSubset{
			"v1": {
				Filter: "Service.Meta.version == v1",
			},
			"v2": {
				Filter: "Service.Meta.version == v2",
			},
		},
		ConnectTimeout: 10 * time.Second,
	}

	success, _, err := configEntries.Set(resolver, nil)
	require.NoError(t, err, "Failed to create service resolver config entry")
	require.True(t, success, "Set should return success")
	defer configEntries.Delete(api.ServiceResolver, serviceName, nil)

	// Wait for the config entry to be visible
	require.True(t, waitForConfigEntry(t, client, api.ServiceResolver, serviceName, 5*time.Second),
		"Service resolver config entry should be visible within timeout")

	// Query the discovery chain — should now reflect the resolver
	resp, qm, err := client.DiscoveryChain().Get(serviceName, nil, nil)
	require.NoError(t, err, "Discovery chain get should succeed after creating resolver")
	require.NotNil(t, resp, "Response should not be nil")
	assert.NotNil(t, qm, "QueryMeta should not be nil")

	chain := resp.Chain
	require.NotNil(t, chain, "Chain should not be nil when resolver exists")
	assert.Equal(t, serviceName, chain.ServiceName, "Chain.ServiceName must match")

	// With a resolver config entry, Default should be false (chain is customized)
	assert.False(t, chain.Default,
		"Chain.Default should be false when a service-resolver exists")

	// The chain must have nodes to traverse
	require.NotEmpty(t, chain.StartNode, "StartNode must not be empty")
	require.NotNil(t, chain.Nodes, "Nodes map must not be nil")
	assert.Contains(t, chain.Nodes, chain.StartNode,
		"Nodes map must contain the StartNode")

	// The chain must have at least one target
	require.NotNil(t, chain.Targets, "Targets map must not be nil")
	assert.NotEmpty(t, chain.Targets, "Targets must not be empty")

	t.Logf("Discovery chain for %q: StartNode=%s, Nodes=%d, Targets=%d, Default=%v",
		serviceName, chain.StartNode, len(chain.Nodes), len(chain.Targets), chain.Default)
}

// CDIS-003: Test discovery chain structure validation — node types and target fields
func TestDiscoveryChainStructure(t *testing.T) {
	client := getClient(t)
	configEntries := client.ConfigEntries()
	serviceName := "cdis-structure-" + randomString(8)

	// Create a service-resolver with subsets to produce a richer chain
	resolver := &api.ServiceResolverConfigEntry{
		Kind:          api.ServiceResolver,
		Name:          serviceName,
		DefaultSubset: "stable",
		Subsets: map[string]api.ServiceResolverSubset{
			"stable": {
				Filter: "Service.Meta.version == v1",
			},
		},
		ConnectTimeout: 5 * time.Second,
	}

	success, _, err := configEntries.Set(resolver, nil)
	require.NoError(t, err)
	require.True(t, success)
	defer configEntries.Delete(api.ServiceResolver, serviceName, nil)

	require.True(t, waitForConfigEntry(t, client, api.ServiceResolver, serviceName, 5*time.Second))

	resp, _, err := client.DiscoveryChain().Get(serviceName, nil, nil)
	require.NoError(t, err)
	require.NotNil(t, resp)
	require.NotNil(t, resp.Chain)

	chain := resp.Chain

	// Validate Chain-level fields
	assert.Equal(t, serviceName, chain.ServiceName, "ServiceName must match")
	assert.NotEmpty(t, chain.StartNode, "StartNode must not be empty")
	assert.NotNil(t, chain.Nodes, "Nodes must not be nil")
	assert.NotNil(t, chain.Targets, "Targets must not be nil")

	// Validate each node in the Nodes map
	validNodeTypes := map[string]bool{
		api.DiscoveryGraphNodeTypeRouter:   true,
		api.DiscoveryGraphNodeTypeSplitter: true,
		api.DiscoveryGraphNodeTypeResolver: true,
	}
	for nodeName, node := range chain.Nodes {
		assert.NotEmpty(t, node.Type, "Node %q Type must not be empty", nodeName)
		assert.True(t, validNodeTypes[node.Type],
			"Node %q has invalid Type %q", nodeName, node.Type)
		assert.NotEmpty(t, node.Name, "Node %q Name must not be empty", nodeName)

		// Type-specific field validation
		switch node.Type {
		case api.DiscoveryGraphNodeTypeResolver:
			assert.NotNil(t, node.Resolver,
				"Resolver node %q must have Resolver field set", nodeName)
		case api.DiscoveryGraphNodeTypeRouter:
			assert.NotNil(t, node.Routes,
				"Router node %q must have Routes field set", nodeName)
		case api.DiscoveryGraphNodeTypeSplitter:
			assert.NotNil(t, node.Splits,
				"Splitter node %q must have Splits field set", nodeName)
		}
	}

	// Validate each target in the Targets map
	for targetName, target := range chain.Targets {
		assert.NotEmpty(t, target.ID,
			"Target %q ID must not be empty", targetName)
		assert.NotEmpty(t, target.Service,
			"Target %q Service must not be empty", targetName)
		assert.NotEmpty(t, target.Name,
			"Target %q Name must not be empty", targetName)
	}

	t.Logf("Chain structure: %d nodes, %d targets, StartNode=%q",
		len(chain.Nodes), len(chain.Targets), chain.StartNode)
}
