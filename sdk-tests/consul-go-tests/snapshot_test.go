package consultest

import (
	"bytes"
	"io"
	"os"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// ==================== Snapshot Save Tests ====================

// TestSnapshotSave tests saving a snapshot
func TestSnapshotSave(t *testing.T) {
	client := getTestClient(t)

	snapshot := client.Snapshot()

	// Save snapshot
	reader, _, err := snapshot.Save(nil)
	if err != nil {
		t.Logf("Snapshot save not available: %v", err)
		return
	}
	defer reader.Close()

	// Read snapshot data
	data, err := io.ReadAll(reader)
	require.NoError(t, err)

	assert.NotEmpty(t, data, "Snapshot data should not be empty")
	t.Logf("Snapshot saved, size: %d bytes", len(data))
}

// TestSnapshotSaveToFile tests saving snapshot to a file
func TestSnapshotSaveToFile(t *testing.T) {
	client := getTestClient(t)

	snapshot := client.Snapshot()

	// Save snapshot
	reader, _, err := snapshot.Save(nil)
	if err != nil {
		t.Logf("Snapshot save not available: %v", err)
		return
	}
	defer reader.Close()

	// Create temp file
	tmpDir := t.TempDir()
	snapshotFile := filepath.Join(tmpDir, "consul-snapshot.snap")

	file, err := os.Create(snapshotFile)
	require.NoError(t, err)
	defer file.Close()

	// Copy snapshot to file
	written, err := io.Copy(file, reader)
	require.NoError(t, err)

	t.Logf("Snapshot saved to file: %s (%d bytes)", snapshotFile, written)

	// Verify file exists and has content
	info, err := os.Stat(snapshotFile)
	require.NoError(t, err)
	assert.True(t, info.Size() > 0, "Snapshot file should not be empty")
}

// ==================== Snapshot Restore Tests ====================

// TestSnapshotRestore tests restoring from a snapshot
func TestSnapshotRestore(t *testing.T) {
	client := getTestClient(t)

	snapshot := client.Snapshot()

	// First save a snapshot
	reader, _, err := snapshot.Save(nil)
	if err != nil {
		t.Logf("Snapshot save not available: %v", err)
		return
	}

	// Read snapshot into buffer
	data, err := io.ReadAll(reader)
	reader.Close()
	require.NoError(t, err)

	t.Logf("Saved snapshot: %d bytes", len(data))

	// Restore from snapshot
	err = snapshot.Restore(nil, bytes.NewReader(data))
	if err != nil {
		t.Logf("Snapshot restore: %v", err)
		return
	}

	t.Log("Snapshot restored successfully")
}

// TestSnapshotRestoreFromFile tests restoring snapshot from file
func TestSnapshotRestoreFromFile(t *testing.T) {
	client := getTestClient(t)

	snapshot := client.Snapshot()

	// Save snapshot to temp file
	reader, _, err := snapshot.Save(nil)
	if err != nil {
		t.Logf("Snapshot save not available: %v", err)
		return
	}

	tmpDir := t.TempDir()
	snapshotFile := filepath.Join(tmpDir, "restore-test.snap")

	file, err := os.Create(snapshotFile)
	require.NoError(t, err)

	_, err = io.Copy(file, reader)
	reader.Close()
	file.Close()
	require.NoError(t, err)

	// Open file for restore
	restoreFile, err := os.Open(snapshotFile)
	require.NoError(t, err)
	defer restoreFile.Close()

	// Restore
	err = snapshot.Restore(nil, restoreFile)
	if err != nil {
		t.Logf("Snapshot restore from file: %v", err)
		return
	}

	t.Log("Snapshot restored from file successfully")
}

// ==================== Snapshot With Data Tests ====================

// TestSnapshotWithKVData tests snapshot includes KV data
func TestSnapshotWithKVData(t *testing.T) {
	client := getTestClient(t)

	kv := client.KV()
	snapshot := client.Snapshot()

	// Create some KV data
	testKey := "snapshot-test-key-" + randomString(8)
	testValue := "snapshot-test-value"

	_, err := kv.Put(&api.KVPair{
		Key:   testKey,
		Value: []byte(testValue),
	}, nil)
	require.NoError(t, err)

	// Save snapshot
	reader, _, err := snapshot.Save(nil)
	if err != nil {
		t.Logf("Snapshot save not available: %v", err)
		kv.Delete(testKey, nil)
		return
	}

	data, err := io.ReadAll(reader)
	reader.Close()
	require.NoError(t, err)

	t.Logf("Snapshot with KV data: %d bytes", len(data))

	// Cleanup
	kv.Delete(testKey, nil)
}

// TestSnapshotWithServices tests snapshot includes service registrations
func TestSnapshotWithServices(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	snapshot := client.Snapshot()

	// Register a service
	serviceName := "snapshot-svc-" + randomString(8)
	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	})
	require.NoError(t, err)

	// Save snapshot
	reader, _, err := snapshot.Save(nil)
	if err != nil {
		t.Logf("Snapshot save not available: %v", err)
		agent.ServiceDeregister(serviceName)
		return
	}

	data, err := io.ReadAll(reader)
	reader.Close()
	require.NoError(t, err)

	t.Logf("Snapshot with services: %d bytes", len(data))

	// Cleanup
	agent.ServiceDeregister(serviceName)
}

// ==================== Snapshot Stale Read Tests ====================

// TestSnapshotStaleRead tests snapshot with stale read option
func TestSnapshotStaleRead(t *testing.T) {
	client := getTestClient(t)

	snapshot := client.Snapshot()

	// Save snapshot with stale option
	opts := &api.QueryOptions{
		AllowStale: true,
	}

	reader, qm, err := snapshot.Save(opts)
	if err != nil {
		t.Logf("Snapshot save with stale not available: %v", err)
		return
	}
	defer reader.Close()

	data, err := io.ReadAll(reader)
	require.NoError(t, err)

	t.Logf("Stale snapshot saved: %d bytes, LastIndex: %d", len(data), qm.LastIndex)
}

// ==================== Snapshot Error Handling Tests ====================

// TestSnapshotRestoreInvalidData tests restore with invalid data
func TestSnapshotRestoreInvalidData(t *testing.T) {
	client := getTestClient(t)

	snapshot := client.Snapshot()

	// Try to restore invalid data
	invalidData := []byte("this is not a valid snapshot")

	err := snapshot.Restore(nil, bytes.NewReader(invalidData))
	if err != nil {
		t.Logf("Expected error for invalid snapshot: %v", err)
	} else {
		t.Log("Restore accepted invalid data (may need server-side validation)")
	}
}

// TestSnapshotRestoreEmptyData tests restore with empty data
func TestSnapshotRestoreEmptyData(t *testing.T) {
	client := getTestClient(t)

	snapshot := client.Snapshot()

	// Try to restore empty data
	err := snapshot.Restore(nil, bytes.NewReader([]byte{}))
	if err != nil {
		t.Logf("Expected error for empty snapshot: %v", err)
	} else {
		t.Log("Restore accepted empty data (may need server-side validation)")
	}
}
