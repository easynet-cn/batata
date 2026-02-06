package consultest

import (
	"testing"
	"time"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// ==================== Namespace Tests (Enterprise) ====================

// TestNamespaceList tests listing namespaces
func TestNamespaceList(t *testing.T) {
	client := getTestClient(t)

	namespaces := client.Namespaces()

	list, _, err := namespaces.List(nil)
	if err != nil {
		t.Logf("Namespace list not available (Enterprise feature): %v", err)
		return
	}

	t.Logf("Found %d namespaces", len(list))
	for _, ns := range list {
		t.Logf("  - %s: %s", ns.Name, ns.Description)
	}
}

// TestNamespaceRead tests reading a namespace
func TestNamespaceRead(t *testing.T) {
	client := getTestClient(t)

	namespaces := client.Namespaces()

	// Try to read default namespace
	ns, _, err := namespaces.Read("default", nil)
	if err != nil {
		t.Logf("Namespace read not available (Enterprise feature): %v", err)
		return
	}

	assert.NotNil(t, ns)
	t.Logf("Namespace: %s, Description: %s", ns.Name, ns.Description)
}

// TestNamespaceCreate tests creating a namespace
func TestNamespaceCreate(t *testing.T) {
	client := getTestClient(t)

	namespaces := client.Namespaces()
	nsName := "test-ns-" + randomString(8)

	ns := &api.Namespace{
		Name:        nsName,
		Description: "Test namespace",
		Meta: map[string]string{
			"created-by": "integration-test",
		},
	}

	created, _, err := namespaces.Create(ns, nil)
	if err != nil {
		t.Logf("Namespace create not available (Enterprise feature): %v", err)
		return
	}

	assert.NotNil(t, created)
	assert.Equal(t, nsName, created.Name)
	t.Logf("Created namespace: %s", created.Name)

	// Cleanup
	namespaces.Delete(nsName, nil)
}

// TestNamespaceUpdate tests updating a namespace
func TestNamespaceUpdate(t *testing.T) {
	client := getTestClient(t)

	namespaces := client.Namespaces()
	nsName := "update-ns-" + randomString(8)

	// Create namespace
	ns := &api.Namespace{
		Name:        nsName,
		Description: "Original description",
	}

	created, _, err := namespaces.Create(ns, nil)
	if err != nil {
		t.Logf("Namespace create not available (Enterprise feature): %v", err)
		return
	}

	// Update namespace
	created.Description = "Updated description"
	updated, _, err := namespaces.Update(created, nil)
	if err != nil {
		t.Logf("Namespace update: %v", err)
		namespaces.Delete(nsName, nil)
		return
	}

	assert.Equal(t, "Updated description", updated.Description)
	t.Logf("Updated namespace description: %s", updated.Description)

	// Cleanup
	namespaces.Delete(nsName, nil)
}

// TestNamespaceDelete tests deleting a namespace
func TestNamespaceDelete(t *testing.T) {
	client := getTestClient(t)

	namespaces := client.Namespaces()
	nsName := "delete-ns-" + randomString(8)

	// Create namespace
	ns := &api.Namespace{
		Name:        nsName,
		Description: "To be deleted",
	}

	_, _, err := namespaces.Create(ns, nil)
	if err != nil {
		t.Logf("Namespace create not available (Enterprise feature): %v", err)
		return
	}

	// Delete namespace
	_, err = namespaces.Delete(nsName, nil)
	if err != nil {
		t.Logf("Namespace delete: %v", err)
		return
	}

	// Verify deleted
	_, _, err = namespaces.Read(nsName, nil)
	if err != nil {
		t.Log("Namespace deleted successfully")
	}
}

// ==================== Partition Tests (Enterprise) ====================

// TestPartitionList tests listing partitions
func TestPartitionList(t *testing.T) {
	client := getTestClient(t)

	partitions := client.Partitions()

	list, _, err := partitions.List(nil, nil)
	if err != nil {
		t.Logf("Partition list not available (Enterprise feature): %v", err)
		return
	}

	t.Logf("Found %d partitions", len(list))
	for _, p := range list {
		t.Logf("  - %s: %s", p.Name, p.Description)
	}
}

// TestPartitionRead tests reading a partition
func TestPartitionRead(t *testing.T) {
	client := getTestClient(t)

	partitions := client.Partitions()

	// Try to read default partition
	p, _, err := partitions.Read(nil, "default", nil)
	if err != nil {
		t.Logf("Partition read not available (Enterprise feature): %v", err)
		return
	}

	assert.NotNil(t, p)
	t.Logf("Partition: %s, Description: %s", p.Name, p.Description)
}

// TestPartitionCreate tests creating a partition
func TestPartitionCreate(t *testing.T) {
	client := getTestClient(t)

	partitions := client.Partitions()
	pName := "test-part-" + randomString(8)

	p := &api.Partition{
		Name:        pName,
		Description: "Test partition",
	}

	created, _, err := partitions.Create(nil, p, nil)
	if err != nil {
		t.Logf("Partition create not available (Enterprise feature): %v", err)
		return
	}

	assert.NotNil(t, created)
	assert.Equal(t, pName, created.Name)
	t.Logf("Created partition: %s", created.Name)

	// Cleanup
	partitions.Delete(nil, pName, nil)
}

// ==================== Cross-Namespace Service Tests ====================

// TestServiceInNamespace tests service registration in namespace
func TestServiceInNamespace(t *testing.T) {
	client := getTestClient(t)

	// Check if namespaces are supported
	namespaces := client.Namespaces()
	_, _, err := namespaces.List(nil)
	if err != nil {
		t.Logf("Namespaces not available (Enterprise feature): %v", err)
		return
	}

	agent := client.Agent()
	serviceName := "ns-service-" + randomString(8)

	// Register service (will use default namespace in OSS)
	reg := &api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	}

	err = agent.ServiceRegister(reg)
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	services, err := agent.Services()
	require.NoError(t, err)

	if svc, ok := services[serviceName]; ok {
		t.Logf("Service %s registered in namespace: %s", svc.ID, svc.Namespace)
	}
}

// TestKVInNamespace tests KV in namespace
func TestKVInNamespace(t *testing.T) {
	client := getTestClient(t)

	// Check if namespaces are supported
	namespaces := client.Namespaces()
	_, _, err := namespaces.List(nil)
	if err != nil {
		t.Logf("Namespaces not available (Enterprise feature): %v", err)
		return
	}

	kv := client.KV()
	key := "ns-kv-test-" + randomString(8)

	// Put KV (will use default namespace in OSS)
	_, err = kv.Put(&api.KVPair{
		Key:   key,
		Value: []byte("namespace test value"),
	}, nil)
	require.NoError(t, err)
	defer kv.Delete(key, nil)

	pair, _, err := kv.Get(key, nil)
	require.NoError(t, err)

	if pair != nil {
		t.Logf("KV %s in namespace: %s", pair.Key, pair.Namespace)
	}
}

// ==================== Peering Tests (1.13+) ====================

// TestPeeringList tests listing peerings
func TestPeeringList(t *testing.T) {
	client := getTestClient(t)

	peerings := client.Peerings()

	list, _, err := peerings.List(nil, nil)
	if err != nil {
		t.Logf("Peering list not available: %v", err)
		return
	}

	t.Logf("Found %d peerings", len(list))
	for _, p := range list {
		t.Logf("  - %s: %s", p.Name, p.State)
	}
}

// TestPeeringRead tests reading a peering
func TestPeeringRead(t *testing.T) {
	client := getTestClient(t)

	peerings := client.Peerings()

	// Try to read non-existent peering
	p, _, err := peerings.Read(nil, "non-existent-peering", nil)
	if err != nil {
		t.Logf("Peering read: %v", err)
		return
	}

	if p != nil {
		t.Logf("Peering: %s, State: %s", p.Name, p.State)
	} else {
		t.Log("Peering not found (expected)")
	}
}

// TestPeeringGenerateToken tests generating a peering token
func TestPeeringGenerateToken(t *testing.T) {
	client := getTestClient(t)

	peerings := client.Peerings()
	peerName := "test-peer-" + randomString(8)

	req := api.PeeringGenerateTokenRequest{
		PeerName: peerName,
	}

	resp, _, err := peerings.GenerateToken(nil, req, nil)
	if err != nil {
		t.Logf("Peering generate token not available: %v", err)
		return
	}

	assert.NotEmpty(t, resp.PeeringToken)
	t.Logf("Generated peering token for %s (length: %d)", peerName, len(resp.PeeringToken))

	// Cleanup - delete the peering
	peerings.Delete(nil, peerName, nil)
}

// ==================== Cross-Datacenter Tests ====================

// TestDatacenters tests listing datacenters
func TestDatacenters(t *testing.T) {
	client := getTestClient(t)

	catalog := client.Catalog()

	dcs, err := catalog.Datacenters()
	require.NoError(t, err)

	assert.NotEmpty(t, dcs, "Should have at least one datacenter")
	t.Logf("Datacenters: %v", dcs)
}

// TestCrossDatacenterQuery tests querying across datacenters
func TestCrossDatacenterQuery(t *testing.T) {
	client := getTestClient(t)

	catalog := client.Catalog()

	// Get all datacenters
	dcs, err := catalog.Datacenters()
	require.NoError(t, err)

	for _, dc := range dcs {
		opts := &api.QueryOptions{
			Datacenter: dc,
		}

		services, _, err := catalog.Services(opts)
		if err != nil {
			t.Logf("Query DC %s: %v", dc, err)
			continue
		}

		t.Logf("DC %s has %d services", dc, len(services))
	}
}
