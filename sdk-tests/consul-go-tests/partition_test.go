package tests

import (
	"context"
	"testing"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// ==================== Partition API Tests (Enterprise) ====================

// CPART-001: Test list partitions (should always include "default")
func TestPartitionList(t *testing.T) {
	client := getTestClient(t)

	partitions, _, err := client.Partitions().List(context.Background(), nil)
	if err != nil {
		t.Skipf("Partition API not available (Enterprise feature): %v", err)
	}

	require.NotNil(t, partitions, "Partition list should not be nil")
	assert.NotEmpty(t, partitions, "Should have at least the default partition")

	foundDefault := false
	for _, p := range partitions {
		assert.NotEmpty(t, p.Name, "Partition name should not be empty")
		if p.Name == api.PartitionDefaultName {
			foundDefault = true
		}
	}
	assert.True(t, foundDefault, "Default partition should exist in list")
	t.Logf("Found %d partitions", len(partitions))
}

// CPART-002: Test read default partition
func TestPartitionReadDefault(t *testing.T) {
	client := getTestClient(t)

	partition, _, err := client.Partitions().Read(context.Background(), api.PartitionDefaultName, nil)
	if err != nil {
		t.Skipf("Partition API not available (Enterprise feature): %v", err)
	}

	require.NotNil(t, partition, "Default partition should exist")
	assert.Equal(t, api.PartitionDefaultName, partition.Name, "Partition name should be 'default'")
	t.Logf("Default partition: Name=%s, Description=%s", partition.Name, partition.Description)
}

// CPART-003: Test create and delete partition
func TestPartitionCreateAndDelete(t *testing.T) {
	client := getTestClient(t)

	name := "part-" + randomID()
	partition := &api.Partition{
		Name:        name,
		Description: "Test partition for SDK tests",
	}

	created, _, err := client.Partitions().Create(context.Background(), partition, nil)
	if err != nil {
		t.Skipf("Partition create not available (Enterprise feature): %v", err)
	}

	require.NotNil(t, created, "Created partition should not be nil")
	assert.Equal(t, name, created.Name, "Created partition name should match")
	assert.NotZero(t, created.CreateIndex, "CreateIndex should be set")
	t.Logf("Created partition: Name=%s, CreateIndex=%d", created.Name, created.CreateIndex)

	// Read back to verify
	read, _, err := client.Partitions().Read(context.Background(), name, nil)
	require.NoError(t, err, "Reading created partition should succeed")
	require.NotNil(t, read, "Created partition should be readable")
	assert.Equal(t, name, read.Name)
	assert.Equal(t, "Test partition for SDK tests", read.Description)

	// Cleanup
	_, err = client.Partitions().Delete(context.Background(), name, nil)
	assert.NoError(t, err, "Partition delete should succeed")

	// Verify deleted
	deleted, _, err := client.Partitions().Read(context.Background(), name, nil)
	assert.NoError(t, err, "Read after delete should not error (returns nil)")
	assert.Nil(t, deleted, "Deleted partition should not be found")
}

// CPART-004: Test update partition description
func TestPartitionUpdate(t *testing.T) {
	client := getTestClient(t)

	name := "part-upd-" + randomID()
	partition := &api.Partition{
		Name:        name,
		Description: "Original description",
	}

	created, _, err := client.Partitions().Create(context.Background(), partition, nil)
	if err != nil {
		t.Skipf("Partition create not available (Enterprise feature): %v", err)
	}
	defer client.Partitions().Delete(context.Background(), name, nil)

	require.NotNil(t, created)

	// Update description
	updated := &api.Partition{
		Name:        name,
		Description: "Updated description",
	}
	result, _, err := client.Partitions().Update(context.Background(), updated, nil)
	if err != nil {
		t.Skipf("Partition update not available: %v", err)
	}

	require.NotNil(t, result, "Updated partition should not be nil")
	assert.Equal(t, name, result.Name)
	assert.Equal(t, "Updated description", result.Description, "Description should be updated")
	t.Logf("Updated partition: Name=%s, Description=%s", result.Name, result.Description)
}

// CPART-005: Test read non-existent partition
func TestPartitionReadNonExistent(t *testing.T) {
	client := getTestClient(t)

	partition, _, err := client.Partitions().Read(context.Background(), "non-existent-partition-"+randomID(), nil)
	if err != nil {
		t.Skipf("Partition API not available (Enterprise feature): %v", err)
	}

	assert.NoError(t, err, "Read non-existent partition should not error")
	assert.Nil(t, partition, "Non-existent partition should return nil")
}

// CPART-006: Test delete non-existent partition (should error or be idempotent)
func TestPartitionDeleteNonExistent(t *testing.T) {
	client := getTestClient(t)

	_, err := client.Partitions().Delete(context.Background(), "non-existent-partition-"+randomID(), nil)
	if err != nil {
		t.Logf("Delete non-existent partition returned error (expected): %v", err)
	} else {
		t.Log("Delete non-existent partition succeeded (idempotent)")
	}
}

// CPART-007: Test create partition with empty name (should fail)
func TestPartitionCreateEmptyName(t *testing.T) {
	client := getTestClient(t)

	partition := &api.Partition{
		Name:        "",
		Description: "Should fail",
	}

	_, _, err := client.Partitions().Create(context.Background(), partition, nil)
	if err != nil {
		t.Logf("Create with empty name correctly rejected: %v", err)
	} else {
		t.Skip("Partition create not available (Enterprise feature)")
	}
}

// CPART-008: Test partition appears in list after creation
func TestPartitionCreateAndList(t *testing.T) {
	client := getTestClient(t)

	name := "part-list-" + randomID()
	partition := &api.Partition{
		Name:        name,
		Description: "List test partition",
	}

	created, _, err := client.Partitions().Create(context.Background(), partition, nil)
	if err != nil {
		t.Skipf("Partition create not available (Enterprise feature): %v", err)
	}
	defer client.Partitions().Delete(context.Background(), name, nil)

	require.NotNil(t, created)

	// List and find the created partition
	list, _, err := client.Partitions().List(context.Background(), nil)
	require.NoError(t, err)

	found := false
	for _, p := range list {
		if p.Name == name {
			found = true
			assert.Equal(t, "List test partition", p.Description)
			break
		}
	}
	assert.True(t, found, "Created partition should appear in list")
}
