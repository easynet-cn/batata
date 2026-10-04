package tests

import (
	"context"
	"testing"
	"time"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// ==================== Debug API Tests ====================
//
// The debug/pprof endpoints require the agent to have enable_debug=true.
// If debug is not enabled, these endpoints return an error and the tests
// will skip gracefully.

// CDBG-001: Test Debug Heap() returns a valid pprof heap dump
func TestDebugHeapDump(t *testing.T) {
	client := getClient(t)

	heap, err := client.Debug().Heap()
	if err != nil {
		t.Skipf("Debug Heap endpoint not available (enable_debug may be false): %v", err)
	}

	require.NoError(t, err, "Heap should succeed when debug is enabled")
	require.NotNil(t, heap, "Heap dump should not be nil")
	assert.NotEmpty(t, heap, "Heap dump should not be empty")
	t.Logf("Heap dump size: %d bytes", len(heap))
}

// CDBG-002: Test Debug Profile() with a short duration (1 second)
func TestDebugCPUProfile(t *testing.T) {
	client := getClient(t)

	// Capture a 1-second CPU profile
	profile, err := client.Debug().Profile(1)
	if err != nil {
		t.Skipf("Debug Profile endpoint not available (enable_debug may be false): %v", err)
	}

	require.NoError(t, err, "Profile should succeed when debug is enabled")
	require.NotNil(t, profile, "Profile data should not be nil")
	// A 1-second CPU profile should produce some data
	assert.NotEmpty(t, profile, "Profile data should not be empty")
	t.Logf("CPU profile size: %d bytes", len(profile))
}

// CDBG-003: Test Debug Goroutine() returns goroutine profile
func TestDebugGoroutineDump(t *testing.T) {
	client := getClient(t)

	goroutines, err := client.Debug().Goroutine()
	if err != nil {
		t.Skipf("Debug Goroutine endpoint not available (enable_debug may be false): %v", err)
	}

	require.NoError(t, err, "Goroutine should succeed when debug is enabled")
	require.NotNil(t, goroutines, "Goroutine dump should not be nil")
	assert.NotEmpty(t, goroutines, "Goroutine dump should not be empty")
	t.Logf("Goroutine dump size: %d bytes", len(goroutines))
}

// CDBG-004: Test Debug PProf() with context and timeout
func TestDebugPProfWithContext(t *testing.T) {
	client := getClient(t)

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	// Capture a 1-second heap profile via PProf
	rc, err := client.Debug().PProf(ctx, "heap", 1)
	if err != nil {
		t.Skipf("Debug PProf endpoint not available (enable_debug may be false): %v", err)
	}

	require.NoError(t, err, "PProf should succeed when debug is enabled")
	require.NotNil(t, rc, "PProf ReadCloser should not be nil")
	defer rc.Close()

	// Read some bytes to verify the stream works
	buf := make([]byte, 1024)
	n, err := rc.Read(buf)
	assert.NoError(t, err, "Reading from PProf stream should succeed")
	assert.Greater(t, n, 0, "Should read at least 1 byte from PProf stream")
	t.Logf("Read %d bytes from PProf stream", n)
}
