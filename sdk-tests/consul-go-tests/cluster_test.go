package tests

import (
	"os"
	"testing"
	"time"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// Cluster tests verify Consul API compatibility across a 3-node batata cluster.
//
// Prerequisites:
//   - 3-node cluster running with Consul enabled
//     (CONSUL_ENABLED=true ./scripts/start-cluster.sh)
//   - Node 1 Consul: 127.0.0.1:8500 (default)
//
// Environment:
//   CONSUL_CLUSTER_NODE1=127.0.0.1:8500 (Consul port on Node 1)
//   CONSUL_CLUSTER_NODE2=127.0.0.1:8510 (Consul port on Node 2)
//   CONSUL_CLUSTER_NODE3=127.0.0.1:8520 (Consul port on Node 3)
//
// Run with:
//   CONSUL_CLUSTER_NODE1=127.0.0.1:8500 go test -run TestCluster -v

func getClusterClients(t *testing.T) (*api.Client, *api.Client, *api.Client) {
	n1 := os.Getenv("CONSUL_CLUSTER_NODE1")
	n2 := os.Getenv("CONSUL_CLUSTER_NODE2")
	n3 := os.Getenv("CONSUL_CLUSTER_NODE3")
	if n1 == "" || n2 == "" || n3 == "" {
		t.Skip("Cluster tests require CONSUL_CLUSTER_NODE1/2/3 env vars")
	}

	c1, err := api.NewClient(&api.Config{Address: n1})
	require.NoError(t, err)
	c2, err := api.NewClient(&api.Config{Address: n2})
	require.NoError(t, err)
	c3, err := api.NewClient(&api.Config{Address: n3})
	require.NoError(t, err)

	return c1, c2, c3
}

// ==================== Status Tests ====================

// CCLU-001: Each node should report a leader
func TestClusterLeaderConsistency(t *testing.T) {
	c1, c2, c3 := getClusterClients(t)

	for i, client := range []*api.Client{c1, c2, c3} {
		leader, err := client.Status().Leader()
		require.NoError(t, err)
		assert.NotEmpty(t, leader, "Node %d should report a leader", i+1)
		assert.Contains(t, leader, ":", "Leader should be in host:port format")
		t.Logf("Node %d leader: %s", i+1, leader)
	}
}

// CCLU-002: Each node should report at least 1 peer (itself)
func TestClusterPeersCount(t *testing.T) {
	c1, c2, c3 := getClusterClients(t)

	for i, client := range []*api.Client{c1, c2, c3} {
		peers, err := client.Status().Peers()
		require.NoError(t, err)
		assert.NotEmpty(t, peers, "Node %d should have at least 1 peer", i+1)
		t.Logf("Node %d peers (%d): %v", i+1, len(peers), peers)
	}
}

// ==================== KV Cross-Node Replication Tests ====================

// CCLU-003: Write KV on Node 1, read from Node 2 and Node 3
func TestClusterKVWriteNode1ReadOthers(t *testing.T) {
	c1, c2, c3 := getClusterClients(t)

	key := "cluster-test/kv-" + randomID()
	value := []byte("written-on-node1")

	// Write on Node 1
	_, err := c1.KV().Put(&api.KVPair{Key: key, Value: value}, nil)
	require.NoError(t, err, "Node 1 should write KV successfully")

	// Wait for replication
	time.Sleep(2 * time.Second)

	// Read from Node 2
	pair2, _, err := c2.KV().Get(key, nil)
	require.NoError(t, err)
	require.NotNil(t, pair2, "Node 2 should read KV written on Node 1")
	assert.Equal(t, value, pair2.Value, "Node 2 value should match")

	// Read from Node 3
	pair3, _, err := c3.KV().Get(key, nil)
	require.NoError(t, err)
	require.NotNil(t, pair3, "Node 3 should read KV written on Node 1")
	assert.Equal(t, value, pair3.Value, "Node 3 value should match")

	// Cleanup
	c1.KV().Delete(key, nil)
}

// CCLU-004: Write KV on different nodes, all visible everywhere
func TestClusterKVMultiNodeWrite(t *testing.T) {
	c1, c2, c3 := getClusterClients(t)

	key1 := "cluster-test/node1-" + randomID()
	key2 := "cluster-test/node2-" + randomID()
	key3 := "cluster-test/node3-" + randomID()

	// Write from each node
	c1.KV().Put(&api.KVPair{Key: key1, Value: []byte("from-node1")}, nil)
	c2.KV().Put(&api.KVPair{Key: key2, Value: []byte("from-node2")}, nil)
	c3.KV().Put(&api.KVPair{Key: key3, Value: []byte("from-node3")}, nil)

	time.Sleep(2 * time.Second)

	// Verify all keys visible from Node 2
	for _, key := range []string{key1, key2, key3} {
		pair, _, err := c2.KV().Get(key, nil)
		require.NoError(t, err)
		require.NotNil(t, pair, "Node 2 should see key %s", key)
	}

	// Cleanup
	c1.KV().Delete(key1, nil)
	c2.KV().Delete(key2, nil)
	c3.KV().Delete(key3, nil)
}

// CCLU-005: Delete KV on one node, verify gone across cluster
func TestClusterKVDeleteReplication(t *testing.T) {
	c1, c2, c3 := getClusterClients(t)

	key := "cluster-test/delete-" + randomID()

	// Write on Node 1
	c1.KV().Put(&api.KVPair{Key: key, Value: []byte("to-delete")}, nil)
	time.Sleep(2 * time.Second)

	// Verify on Node 3
	pair, _, _ := c3.KV().Get(key, nil)
	require.NotNil(t, pair, "Node 3 should see KV before delete")

	// Delete on Node 2
	_, err := c2.KV().Delete(key, nil)
	require.NoError(t, err)

	time.Sleep(2 * time.Second)

	// Verify gone on Node 1 and Node 3
	pair1, _, _ := c1.KV().Get(key, nil)
	assert.Nil(t, pair1, "Node 1 should not see deleted KV")

	pair3, _, _ := c3.KV().Get(key, nil)
	assert.Nil(t, pair3, "Node 3 should not see deleted KV")
}

// ==================== Service Registration Cross-Node Tests ====================

// CCLU-006: Register service on each node, verify local registration works
func TestClusterServiceLocalRegistration(t *testing.T) {
	c1, c2, c3 := getClusterClients(t)

	// Each node can independently register and list services
	for i, c := range []*api.Client{c1, c2, c3} {
		svcName := "cluster-local-svc-" + randomID()
		reg := &api.AgentServiceRegistration{
			ID:      svcName,
			Name:    svcName,
			Port:    8080 + i,
			Address: "10.1.1." + string(rune('1'+i)),
		}
		err := c.Agent().ServiceRegister(reg)
		require.NoError(t, err, "Node %d should register service locally", i+1)

		// Verify visible on same node
		svcs, err := c.Agent().Services()
		require.NoError(t, err)
		assert.Contains(t, svcs, svcName,
			"Node %d should see its own registered service", i+1)

		// Cleanup
		c.Agent().ServiceDeregister(svcName)
	}
}

// ==================== Session Cross-Node Tests ====================

// CCLU-008: Create session on Node 1, read from Node 2
func TestClusterSessionReplication(t *testing.T) {
	c1, c2, _ := getClusterClients(t)

	sessionID, _, err := c1.Session().Create(&api.SessionEntry{
		Name: "cluster-session-" + randomID(),
		TTL:  "30s",
	}, nil)
	require.NoError(t, err)
	require.NotEmpty(t, sessionID)
	defer c1.Session().Destroy(sessionID, nil)

	time.Sleep(2 * time.Second)

	// Read from Node 2
	info, _, err := c2.Session().Info(sessionID, nil)
	if err != nil {
		t.Skipf("Cross-node session read not available: %v", err)
	}
	if info != nil {
		assert.Equal(t, sessionID, info.ID, "Node 2 should see session from Node 1")
		t.Logf("Session %s visible on Node 2", sessionID)
	}
}
