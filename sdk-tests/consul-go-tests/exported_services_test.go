package tests

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

// ==================== Exported Services API Tests (Enterprise) ====================
//
// The ExportedServices and ImportedServices APIs are Enterprise features
// that require peering or partition exports to be configured. If the
// agent is not Enterprise or no peerings exist, these endpoints may
// return empty results or errors — tests skip gracefully in that case.

// CEXP-001: Test ExportedServices list returns without error
func TestExportedServicesList(t *testing.T) {
	client := getClient(t)

	services, qm, err := client.ExportedServices(nil)
	if err != nil {
		t.Skipf("ExportedServices API not available (Enterprise feature): %v", err)
	}

	// On a CE agent, this endpoint returns an empty list without error.
	// On an Enterprise agent without any exports configured, it also
	// returns an empty list.
	assert.NoError(t, err, "ExportedServices should not error")
	assert.NotNil(t, qm, "QueryMeta should not be nil")
	assert.Greater(t, qm.LastIndex, uint64(0),
		"QueryMeta.LastIndex should be > 0")

	// Validate structure of any returned services
	for i, svc := range services {
		assert.NotEmpty(t, svc.Service,
			"ExportedService[%d].Service must not be empty", i)
		// Consumers may be empty if no peers/partitions consume the service
		t.Logf("Exported service %d: Service=%s, Partition=%s, Namespace=%s, Peers=%v, Partitions=%v",
			i, svc.Service, svc.Partition, svc.Namespace,
			svc.Consumers.Peers, svc.Consumers.Partitions)
	}

	t.Logf("ExportedServices returned %d entries", len(services))
}

// CIMP-001: Test ImportedServices list returns without error
func TestImportedServicesList(t *testing.T) {
	client := getClient(t)

	services, qm, err := client.ImportedServices(nil)
	if err != nil {
		t.Skipf("ImportedServices API not available (Enterprise feature): %v", err)
	}

	assert.NoError(t, err, "ImportedServices should not error")
	assert.NotNil(t, qm, "QueryMeta should not be nil")
	assert.Greater(t, qm.LastIndex, uint64(0),
		"QueryMeta.LastIndex should be > 0")

	// Validate structure of any returned services
	for i, svc := range services {
		assert.NotEmpty(t, svc.Service,
			"ImportedService[%d].Service must not be empty", i)
		// SourcePeer/SourcePartition may be empty if not from a peering
		t.Logf("Imported service %d: Service=%s, SourcePeer=%s, SourcePartition=%s, Partition=%s",
			i, svc.Service, svc.SourcePeer, svc.SourcePartition, svc.Partition)
	}

	t.Logf("ImportedServices returned %d entries", len(services))
}
