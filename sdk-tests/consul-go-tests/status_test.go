package tests

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

// ==================== Status API Tests ====================

// CST-001: Test get leader
func TestStatusLeader(t *testing.T) {
	client := getClient(t)

	leader, err := client.Status().Leader()
	assert.NoError(t, err, "Status leader should succeed")

	t.Logf("Cluster leader: %s", leader)
	// Leader may be empty in single-node setup
}

// CST-002: Test get peers
func TestStatusPeers(t *testing.T) {
	client := getClient(t)

	peers, err := client.Status().Peers()
	assert.NoError(t, err, "Status peers should succeed")

	t.Logf("Cluster peers: %v", peers)
	// Peers may be empty in single-node setup
}
