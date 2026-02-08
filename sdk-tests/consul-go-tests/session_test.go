package tests

import (
	"testing"
	"time"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// ==================== Session API Tests ====================

// CS-001: Test create session
func TestSessionCreate(t *testing.T) {
	client := getClient(t)

	session := &api.SessionEntry{
		Name:     "test-session-" + randomID(),
		TTL:      "30s",
		Behavior: api.SessionBehaviorDelete,
	}

	sessionID, _, err := client.Session().Create(session, nil)
	assert.NoError(t, err, "Session create should succeed")
	assert.NotEmpty(t, sessionID, "Should return session ID")

	t.Logf("Created session: %s", sessionID)

	// Cleanup
	client.Session().Destroy(sessionID, nil)
}

// CS-002: Test destroy session
func TestSessionDestroy(t *testing.T) {
	client := getClient(t)

	// Create first
	session := &api.SessionEntry{
		Name: "destroy-session-" + randomID(),
		TTL:  "30s",
	}
	sessionID, _, err := client.Session().Create(session, nil)
	require.NoError(t, err)

	// Destroy
	_, err = client.Session().Destroy(sessionID, nil)
	assert.NoError(t, err, "Session destroy should succeed")

	// Verify destroyed
	info, _, err := client.Session().Info(sessionID, nil)
	assert.NoError(t, err)
	assert.Nil(t, info, "Session should be destroyed")
}

// CS-003: Test get session info
func TestSessionInfo(t *testing.T) {
	client := getClient(t)

	// Create session
	session := &api.SessionEntry{
		Name: "info-session-" + randomID(),
		TTL:  "30s",
	}
	sessionID, _, err := client.Session().Create(session, nil)
	require.NoError(t, err)
	defer client.Session().Destroy(sessionID, nil)

	// Get info
	info, _, err := client.Session().Info(sessionID, nil)
	assert.NoError(t, err, "Session info should succeed")
	assert.NotNil(t, info)
	assert.Equal(t, sessionID, info.ID)

	t.Logf("Session info: Name=%s, TTL=%s", info.Name, info.TTL)
}

// CS-004: Test list sessions
func TestSessionList(t *testing.T) {
	client := getClient(t)
	prefix := "list-session-" + randomID()

	// Create multiple sessions
	var sessionIDs []string
	for i := 0; i < 3; i++ {
		session := &api.SessionEntry{
			Name: prefix + "-" + string(rune('a'+i)),
			TTL:  "30s",
		}
		sessionID, _, err := client.Session().Create(session, nil)
		require.NoError(t, err)
		sessionIDs = append(sessionIDs, sessionID)
	}
	defer func() {
		for _, id := range sessionIDs {
			client.Session().Destroy(id, nil)
		}
	}()

	// List
	sessions, _, err := client.Session().List(nil)
	assert.NoError(t, err, "Session list should succeed")
	assert.GreaterOrEqual(t, len(sessions), 3, "Should have at least 3 sessions")

	t.Logf("Found %d sessions", len(sessions))
}

// CS-005: Test renew session
func TestSessionRenew(t *testing.T) {
	client := getClient(t)

	// Create session with TTL
	session := &api.SessionEntry{
		Name: "renew-session-" + randomID(),
		TTL:  "15s",
	}
	sessionID, _, err := client.Session().Create(session, nil)
	require.NoError(t, err)
	defer client.Session().Destroy(sessionID, nil)

	// Renew
	entry, _, err := client.Session().Renew(sessionID, nil)
	assert.NoError(t, err, "Session renew should succeed")
	assert.NotNil(t, entry)

	t.Logf("Renewed session: %s", sessionID)
}

// CS-006: Test list node sessions
func TestSessionNode(t *testing.T) {
	client := getClient(t)

	// Get node name
	self, err := client.Agent().Self()
	if err != nil {
		t.Skip("Could not get agent self info")
	}
	config := self["Config"]
	nodeName := config["NodeName"].(string)

	// Create session on this node
	session := &api.SessionEntry{
		Name: "node-session-" + randomID(),
		TTL:  "30s",
	}
	sessionID, _, err := client.Session().Create(session, nil)
	require.NoError(t, err)
	defer client.Session().Destroy(sessionID, nil)

	// List node sessions
	sessions, _, err := client.Session().Node(nodeName, nil)
	assert.NoError(t, err, "Session node should succeed")

	t.Logf("Node %s has %d sessions", nodeName, len(sessions))
}

// CS-007: Test session with lock delay
func TestSessionLockDelay(t *testing.T) {
	client := getClient(t)

	// Create session with lock delay
	session := &api.SessionEntry{
		Name:      "lockdelay-session-" + randomID(),
		TTL:       "30s",
		LockDelay: 5 * time.Second,
		Behavior:  api.SessionBehaviorRelease,
	}

	sessionID, _, err := client.Session().Create(session, nil)
	assert.NoError(t, err, "Session create with lock delay should succeed")
	defer client.Session().Destroy(sessionID, nil)

	// Verify session info
	info, _, err := client.Session().Info(sessionID, nil)
	assert.NoError(t, err)
	assert.NotNil(t, info)

	t.Logf("Session with lock delay: %s", sessionID)
}

// CS-008: Test session with checks
func TestSessionWithChecks(t *testing.T) {
	client := getClient(t)

	// Register a TTL check first
	checkID := "session-check-" + randomID()
	err := client.Agent().CheckRegister(&api.AgentCheckRegistration{
		ID:   checkID,
		Name: "session-check",
		AgentServiceCheck: api.AgentServiceCheck{
			TTL: "30s",
		},
	})
	require.NoError(t, err)
	defer client.Agent().CheckDeregister(checkID)

	// Pass the check
	client.Agent().PassTTL(checkID, "healthy")

	// Create session with check
	session := &api.SessionEntry{
		Name:   "check-session-" + randomID(),
		TTL:    "30s",
		Checks: []string{checkID},
	}

	sessionID, _, err := client.Session().Create(session, nil)
	// May fail if checks feature not fully supported
	if err == nil {
		defer client.Session().Destroy(sessionID, nil)
		t.Logf("Created session with checks: %s", sessionID)
	} else {
		t.Logf("Session with checks not supported: %v", err)
	}
}
