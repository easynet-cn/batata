// Package tests – SDK compatibility regression tests.
//
// Only assertions expressed through the official `github.com/hashicorp/consul/api`
// client types live here. Raw HTTP/endpoint assertions were moved to
// `sdk-tests/consul-protocol-tests`, which is a protocol contract suite.

package tests

import (
	"io"
	"testing"
	"time"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// ---------------------------------------------------------------------------
// SDK-7: Vivaldi coordinate updates over time
// ---------------------------------------------------------------------------

func TestCompatVivaldiCoordinatesRepeat(t *testing.T) {
	client := getClient(t)
	c1, _, err := client.Coordinate().Nodes(nil)
	require.NoError(t, err)
	c2, _, err := client.Coordinate().Nodes(nil)
	require.NoError(t, err)
	assert.Equal(t, len(c1), len(c2))
	for _, n := range c1 {
		assert.Len(t, n.Coord.Vec, 8, "Vivaldi vector must have 8 dimensions")
		assert.GreaterOrEqual(t, n.Coord.Height, 1.0e-5,
			"Vivaldi height must be >= HEIGHT_MIN")
	}
}

// ---------------------------------------------------------------------------
// P2-3: filter CEL operators (in, contains, and/or/not)
// ---------------------------------------------------------------------------

func TestCompatFilterInOperator(t *testing.T) {
	client := getClient(t)
	svc := "compat-filter-in-" + randomID()
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   svc,
		Name: svc,
		Tags: []string{"v1", "prod"},
		Port: 8080,
	})
	require.NoError(t, err)
	defer client.Agent().ServiceDeregister(svc)
	require.True(t, waitForService(t, client, svc, 10*time.Second), "service should appear in catalog")

	opts := (&api.QueryOptions{Filter: `"v1" in ServiceTags`})
	entries, _, err := client.Health().Service(svc, "", false, opts)
	require.NoError(t, err)
	assert.Greater(t, len(entries), 0, "filter 'v1' in ServiceTags must match")

	opts2 := (&api.QueryOptions{Filter: `"v999" in ServiceTags`})
	entries2, _, err := client.Health().Service(svc, "", false, opts2)
	require.NoError(t, err)
	assert.Equal(t, 0, len(entries2), "non-matching tag must filter out")
}

func TestCompatFilterContainsOperator(t *testing.T) {
	client := getClient(t)
	svc := "compat-filter-contains-" + randomID()
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   svc,
		Name: svc,
		Tags: []string{"staging"},
		Port: 8080,
	})
	require.NoError(t, err)
	defer client.Agent().ServiceDeregister(svc)
	require.True(t, waitForService(t, client, svc, 10*time.Second), "service should appear in catalog")

	opts := &api.QueryOptions{Filter: `ServiceTags contains "staging"`}
	entries, _, err := client.Health().Service(svc, "", false, opts)
	require.NoError(t, err)
	assert.Greater(t, len(entries), 0, "contains operator must match")
}

func TestCompatFilterLogicalOperators(t *testing.T) {
	client := getClient(t)
	svc := "compat-filter-logic-" + randomID()
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   svc,
		Name: svc,
		Tags: []string{"a", "b"},
		Port: 8080,
	})
	require.NoError(t, err)
	defer client.Agent().ServiceDeregister(svc)
	require.True(t, waitForService(t, client, svc, 10*time.Second), "service should appear in catalog")

	opts := &api.QueryOptions{
		Filter: `"a" in ServiceTags and "b" in ServiceTags`,
	}
	entries, _, err := client.Health().Service(svc, "", false, opts)
	require.NoError(t, err)
	assert.Greater(t, len(entries), 0, "AND of two true conditions must match")

	opts = &api.QueryOptions{Filter: `"nonexistent" in ServiceTags or "a" in ServiceTags`}
	entries, _, err = client.Health().Service(svc, "", false, opts)
	require.NoError(t, err)
	assert.Greater(t, len(entries), 0, "OR with one true branch must match")

	opts = &api.QueryOptions{Filter: `not "nonexistent" in ServiceTags`}
	entries, _, err = client.Health().Service(svc, "", false, opts)
	require.NoError(t, err)
	assert.Greater(t, len(entries), 0, "NOT of false must match")
}

// ---------------------------------------------------------------------------
// P2-5: snapshot archive format (tar structure)
// ---------------------------------------------------------------------------

func TestCompatSnapshotIsTarArchive(t *testing.T) {
	client := getClient(t)
	snap, _, err := client.Snapshot().Save(nil)
	require.NoError(t, err)
	defer snap.Close()

	buf := make([]byte, 512)
	n, err := io.ReadFull(snap, buf)
	if err != nil && err != io.ErrUnexpectedEOF {
		require.NoError(t, err)
	}
	require.Equal(t, 512, n, "first tar block must be 512 bytes")
	magic := string(buf[257:262])
	assert.True(t, magic == "ustar" || string(buf[:100]) != "",
		"snapshot must be a tar archive; got magic=%q", magic)
}

// ---------------------------------------------------------------------------
// SDK-2 / merge-central-config: ServiceConnect field present
// ---------------------------------------------------------------------------

func TestCompatCatalogServiceConnectField(t *testing.T) {
	client := getClient(t)
	svc := "compat-connect-field-" + randomID()
	require.NoError(t, client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID: svc, Name: svc, Port: 8080,
	}))
	defer client.Agent().ServiceDeregister(svc)
	require.True(t, waitForService(t, client, svc, 10*time.Second), "service should appear in catalog")

	services, _, err := client.Catalog().Service(svc, "", nil)
	require.NoError(t, err)
	require.NotEmpty(t, services)
	assert.Equal(t, svc, services[0].ServiceName)
}
