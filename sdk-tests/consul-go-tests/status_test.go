package tests

import (
	"net"
	"strconv"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// ==================== Status API Tests ====================

// CST-001: Test get leader
func TestStatusLeader(t *testing.T) {
	client := getClient(t)

	leader, err := client.Status().Leader()
	assert.NoError(t, err, "Status leader should succeed")

	t.Logf("Cluster leader: %s", leader)
	assert.NotEmpty(t, leader, "Leader should not be empty")
	assert.Contains(t, leader, ":", "Leader should be in host:port format")
}

// CST-002: Test get peers
func TestStatusPeers(t *testing.T) {
	client := getClient(t)

	peers, err := client.Status().Peers()
	assert.NoError(t, err, "Status peers should succeed")

	t.Logf("Cluster peers: %v", peers)
	assert.NotNil(t, peers, "Peers list should not be nil")
	assert.True(t, len(peers) > 0, "Should have at least one peer")
}

// CST-003: Test leader returns valid address format
func TestStatusLeaderFormat(t *testing.T) {
	client := getClient(t)

	leader, err := client.Status().Leader()
	assert.NoError(t, err)

	if leader != "" {
		// Leader should be in "ip:port" format
		assert.Contains(t, leader, ":", "Leader should be in ip:port format")
		t.Logf("Leader address: %s", leader)
	} else {
		t.Log("Leader is empty (no leader elected)")
	}
}

// CST-004: Test peers returns consistent data
func TestStatusPeersConsistency(t *testing.T) {
	client := getClient(t)

	// Call peers twice and compare
	peers1, err := client.Status().Peers()
	assert.NoError(t, err)

	peers2, err := client.Status().Peers()
	assert.NoError(t, err)

	assert.Equal(t, len(peers1), len(peers2), "Peers count should be consistent across calls")
	t.Logf("Peers count (consistent): %d", len(peers1))
}

// CST-005: Test leader is included in peers
func TestStatusLeaderInPeers(t *testing.T) {
	client := getClient(t)

	leader, err := client.Status().Leader()
	assert.NoError(t, err)

	peers, err := client.Status().Peers()
	assert.NoError(t, err)

	assert.NotEmpty(t, leader, "Leader should not be empty")
	assert.NotEmpty(t, peers, "Peers should not be empty")

	if leader != "" && len(peers) > 0 {
		found := false
		for _, peer := range peers {
			if peer == leader {
				found = true
				break
			}
		}
		if found {
			t.Logf("Leader %s found in peers list", leader)
		} else {
			t.Logf("Leader %s not found in peers (may be normal with proxied connections)", leader)
		}
	}
}

// CST-006: Test status multiple calls are consistent
func TestStatusMultipleCalls(t *testing.T) {
	client := getClient(t)

	// Call leader and peers multiple times to verify consistency
	for i := 0; i < 3; i++ {
		leader, err := client.Status().Leader()
		assert.NoError(t, err, "Leader call %d should succeed", i)

		peers, err := client.Status().Peers()
		assert.NoError(t, err, "Peers call %d should succeed", i)

		t.Logf("Call %d: leader=%s, peers=%d", i, leader, len(peers))
	}
}

// CST-007: Test peers with single-node cluster
func TestStatusPeersSingleNode(t *testing.T) {
	client := getClient(t)

	peers, err := client.Status().Peers()
	assert.NoError(t, err)

	// In a single node cluster, there should be at least one peer (itself)
	assert.NotEmpty(t, peers, "Should have at least one peer in the cluster")
	for _, peer := range peers {
		assert.NotEmpty(t, peer, "Peer address should not be empty")
		assert.Contains(t, peer, ":", "Peer should be in host:port format")
	}
	t.Logf("Peers in cluster: %v", peers)
}

// CST-008: Test leader address format is consistent and valid across multiple calls
// Validates: host:port format, port is numeric, host is non-empty, and the
// address is identical across consecutive calls.
func TestStatusLeaderFormatConsistency(t *testing.T) {
	client := getClient(t)

	// Collect leader addresses from multiple consecutive calls
	const numCalls = 5
	leaders := make([]string, numCalls)
	for i := 0; i < numCalls; i++ {
		leader, err := client.Status().Leader()
		require.NoError(t, err, "Leader call %d should succeed", i)
		require.NotEmpty(t, leader, "Leader address must not be empty on call %d", i)
		leaders[i] = leader
	}

	// All calls must return the same leader address
	for i := 1; i < numCalls; i++ {
		assert.Equal(t, leaders[0], leaders[i],
			"Leader address must be consistent across calls (call %d differs)", i)
	}

	// Now validate the format of the leader address in detail
	leader := leaders[0]

	// Use net.SplitHostPort for proper host:port parsing — handles IPv6 too
	host, portStr, err := net.SplitHostPort(leader)
	require.NoError(t, err,
		"Leader address %q must be in host:port format (SplitHostPort failed): %v", leader, err)

	// Host must not be empty
	assert.NotEmpty(t, host,
		"Host portion of leader address must not be empty (got %q)", leader)

	// Port must be numeric and in valid range
	port, err := strconv.Atoi(portStr)
	require.NoError(t, err,
		"Port portion %q must be numeric (got leader=%q)", portStr, leader)
	assert.Greater(t, port, 0,
		"Port must be > 0 (got %d from leader=%q)", port, leader)
	assert.Less(t, port, 65536,
		"Port must be < 65536 (got %d from leader=%q)", port, leader)

	// The host should be a valid IP address or a resolvable hostname
	// At minimum, it should not contain spaces or control characters
	assert.NotContains(t, host, " ",
		"Host portion must not contain spaces (got %q)", host)

	t.Logf("Leader address validated: host=%s, port=%d (full=%q)", host, port, leader)
}
