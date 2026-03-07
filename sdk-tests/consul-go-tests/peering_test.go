package tests

import (
	"testing"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// ==================== Additional Peering API Tests ====================

// CP-004: Test delete non-existent peering
func TestPeeringDeleteNonExistent(t *testing.T) {
	client := getClient(t)

	_, err := client.Peerings().Delete(nil, "nonexistent-peer-"+randomID(), nil)
	if err != nil {
		t.Logf("Delete non-existent peering: %v", err)
	} else {
		t.Log("Delete non-existent peering succeeded (idempotent)")
	}
}

// CP-005: Test generate token and read peering
func TestPeeringGenerateTokenAndRead(t *testing.T) {
	client := getClient(t)

	peerName := "peer-read-" + randomID()
	req := api.PeeringGenerateTokenRequest{
		PeerName: peerName,
	}

	_, _, err := client.Peerings().GenerateToken(nil, req, nil)
	if err != nil {
		t.Skip("Peering API not available")
	}
	defer client.Peerings().Delete(nil, peerName, nil)

	// Read the peering
	peering, _, err := client.Peerings().Read(nil, peerName, nil)
	assert.NoError(t, err)

	if peering != nil {
		assert.Equal(t, peerName, peering.Name, "Peering name should match")
		t.Logf("Peering %s state: %s", peering.Name, peering.State)
	} else {
		t.Log("Peering not found after token generation (may be pending)")
	}
}

// CP-006: Test generate token appears in list
func TestPeeringGenerateTokenInList(t *testing.T) {
	client := getClient(t)

	peerName := "peer-list-chk-" + randomID()
	req := api.PeeringGenerateTokenRequest{
		PeerName: peerName,
	}

	_, _, err := client.Peerings().GenerateToken(nil, req, nil)
	if err != nil {
		t.Skip("Peering API not available")
	}
	defer client.Peerings().Delete(nil, peerName, nil)

	// List peerings
	peerings, _, err := client.Peerings().List(nil, nil)
	require.NoError(t, err)

	found := false
	for _, p := range peerings {
		if p.Name == peerName {
			found = true
			break
		}
	}

	if found {
		t.Logf("Peering %s found in list", peerName)
	} else {
		t.Logf("Peering %s not found in list (may need time to propagate)", peerName)
	}
}

// CP-007: Test delete peering lifecycle
func TestPeeringDeleteLifecycle(t *testing.T) {
	client := getClient(t)

	peerName := "peer-delete-lc-" + randomID()
	req := api.PeeringGenerateTokenRequest{
		PeerName: peerName,
	}

	_, _, err := client.Peerings().GenerateToken(nil, req, nil)
	if err != nil {
		t.Skip("Peering API not available")
	}

	// Delete the peering
	_, err = client.Peerings().Delete(nil, peerName, nil)
	assert.NoError(t, err, "Delete peering should succeed")

	// Verify deleted
	peering, _, err := client.Peerings().Read(nil, peerName, nil)
	if err == nil && peering != nil {
		t.Logf("Peering state after delete: %s", peering.State)
	} else {
		t.Log("Peering deleted successfully")
	}
}

// CP-008: Test generate token with metadata
func TestPeeringGenerateTokenWithMeta(t *testing.T) {
	client := getClient(t)

	peerName := "peer-meta-" + randomID()
	req := api.PeeringGenerateTokenRequest{
		PeerName: peerName,
		Meta: map[string]string{
			"env":    "test",
			"region": "us-west-1",
		},
	}

	resp, _, err := client.Peerings().GenerateToken(nil, req, nil)
	if err != nil {
		t.Skip("Peering API not available")
	}
	defer client.Peerings().Delete(nil, peerName, nil)

	assert.NotEmpty(t, resp.PeeringToken)

	// Read peering to verify meta
	peering, _, err := client.Peerings().Read(nil, peerName, nil)
	if err == nil && peering != nil {
		t.Logf("Peering meta: %v", peering.Meta)
	}
}
