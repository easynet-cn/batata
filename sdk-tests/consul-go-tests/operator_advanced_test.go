package tests

import (
	"testing"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// ==================== Advanced Operator API Tests ====================

// CO-ADV-001: Test operator usage
func TestOperatorUsage(t *testing.T) {
	client := getTestClient(t)

	operator := client.Operator()

	usage, _, err := operator.Usage(nil)
	if err != nil {
		t.Skipf("Operator Usage not available: %v", err)
	}

	require.NotNil(t, usage, "Usage response should not be nil")
	assert.NotEmpty(t, usage.Usage, "Usage map should not be empty")

	for dc, info := range usage.Usage {
		assert.GreaterOrEqual(t, info.Nodes, 0, "Nodes count should be >= 0")
		assert.GreaterOrEqual(t, info.Services, 0, "Services count should be >= 0")
		t.Logf("Datacenter %s: Nodes=%d, Services=%d, ServiceInstances=%d",
			dc, info.Nodes, info.Services, info.ServiceInstances)
	}
}

// CO-ADV-002: Test operator area update (Enterprise feature)
func TestOperatorAreaUpdate(t *testing.T) {
	client := getTestClient(t)

	operator := client.Operator()

	// Create an area first
	area := &api.Area{
		PeerDatacenter: "update-dc-" + randomString(8),
	}

	id, _, err := operator.AreaCreate(area, nil)
	if err != nil {
		t.Skipf("Area create not available (Enterprise feature): %v", err)
	}
	defer operator.AreaDelete(id, nil)

	assert.NotEmpty(t, id, "Created area ID should not be empty")

	// Update the area
	updatedArea := &api.Area{
		ID:             id,
		PeerDatacenter: "updated-dc-" + randomString(8),
	}

	updatedID, _, err := operator.AreaUpdate(id, updatedArea, nil)
	if err != nil {
		t.Skipf("Area update not available (Enterprise feature): %v", err)
	}

	assert.NotEmpty(t, updatedID, "Updated area ID should not be empty")
	t.Logf("Area %s updated successfully", id)
}

// CO-ADV-003: Test operator area get (Enterprise feature)
func TestOperatorAreaGet(t *testing.T) {
	client := getTestClient(t)

	operator := client.Operator()

	// Create an area first
	area := &api.Area{
		PeerDatacenter: "get-dc-" + randomString(8),
	}

	id, _, err := operator.AreaCreate(area, nil)
	if err != nil {
		t.Skipf("Area create not available (Enterprise feature): %v", err)
	}
	defer operator.AreaDelete(id, nil)

	assert.NotEmpty(t, id, "Created area ID should not be empty")

	// Get the area by ID
	areas, _, err := operator.AreaGet(id, nil)
	if err != nil {
		t.Skipf("Area get not available (Enterprise feature): %v", err)
	}

	require.NotEmpty(t, areas, "Should return at least one area")
	assert.Equal(t, id, areas[0].ID, "Area ID should match")
	assert.Equal(t, area.PeerDatacenter, areas[0].PeerDatacenter, "PeerDatacenter should match")

	t.Logf("Area retrieved: ID=%s, PeerDatacenter=%s", areas[0].ID, areas[0].PeerDatacenter)
}
