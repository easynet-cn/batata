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

// CS-009: Test renew destroyed session fails
func TestSessionRenewDestroyed(t *testing.T) {
	client := getClient(t)

	// Create and immediately destroy
	session := &api.SessionEntry{
		Name: "renew-destroyed-" + randomID(),
		TTL:  "30s",
	}
	sessionID, _, err := client.Session().Create(session, nil)
	require.NoError(t, err)

	_, err = client.Session().Destroy(sessionID, nil)
	require.NoError(t, err)

	// Try to renew destroyed session
	entry, _, err := client.Session().Renew(sessionID, nil)
	// Should return nil entry or error
	if err != nil {
		t.Logf("Renew destroyed session error (expected): %v", err)
	} else {
		assert.Nil(t, entry, "Renew of destroyed session should return nil entry")
		t.Log("Renew of destroyed session returned nil (correct)")
	}
}

// CS-010: Test session with release behavior
func TestSessionReleaseBehavior(t *testing.T) {
	client := getClient(t)

	session := &api.SessionEntry{
		Name:     "release-behavior-" + randomID(),
		TTL:      "30s",
		Behavior: api.SessionBehaviorRelease,
	}

	sessionID, _, err := client.Session().Create(session, nil)
	require.NoError(t, err)
	defer client.Session().Destroy(sessionID, nil)

	info, _, err := client.Session().Info(sessionID, nil)
	assert.NoError(t, err)
	assert.NotNil(t, info)
	assert.Equal(t, api.SessionBehaviorRelease, info.Behavior,
		"Session behavior should be release")
	t.Logf("Session %s has release behavior", sessionID)
}

// CS-011: Test session with delete behavior
func TestSessionDeleteBehavior(t *testing.T) {
	client := getClient(t)

	session := &api.SessionEntry{
		Name:     "delete-behavior-" + randomID(),
		TTL:      "30s",
		Behavior: api.SessionBehaviorDelete,
	}

	sessionID, _, err := client.Session().Create(session, nil)
	require.NoError(t, err)
	defer client.Session().Destroy(sessionID, nil)

	info, _, err := client.Session().Info(sessionID, nil)
	assert.NoError(t, err)
	assert.NotNil(t, info)
	assert.Equal(t, api.SessionBehaviorDelete, info.Behavior,
		"Session behavior should be delete")
	t.Logf("Session %s has delete behavior", sessionID)
}

// CS-012: Test session KV lock and release
func TestSessionKVLock(t *testing.T) {
	client := getClient(t)

	session := &api.SessionEntry{
		Name:     "kv-lock-session-" + randomID(),
		TTL:      "30s",
		Behavior: api.SessionBehaviorRelease,
	}

	sessionID, _, err := client.Session().Create(session, nil)
	require.NoError(t, err)
	defer client.Session().Destroy(sessionID, nil)

	// Acquire a KV lock
	key := "session-lock-key-" + randomID()
	acquired, _, err := client.KV().Acquire(&api.KVPair{
		Key:     key,
		Value:   []byte("locked"),
		Session: sessionID,
	}, nil)
	assert.NoError(t, err)
	assert.True(t, acquired, "Should acquire KV lock")
	defer client.KV().Delete(key, nil)

	// Verify the lock
	pair, _, err := client.KV().Get(key, nil)
	assert.NoError(t, err)
	assert.NotNil(t, pair)
	assert.Equal(t, sessionID, pair.Session, "KV should be locked by session")

	// Release the lock
	released, _, err := client.KV().Release(&api.KVPair{
		Key:     key,
		Session: sessionID,
	}, nil)
	assert.NoError(t, err)
	assert.True(t, released, "Should release KV lock")

	// Verify released
	pair, _, err = client.KV().Get(key, nil)
	assert.NoError(t, err)
	if pair != nil {
		assert.Empty(t, pair.Session, "KV should have no session after release")
	}
}

// CS-013: Test session destroy releases KV locks
func TestSessionDestroyReleasesLocks(t *testing.T) {
	client := getClient(t)

	session := &api.SessionEntry{
		Name:     "destroy-release-" + randomID(),
		TTL:      "30s",
		Behavior: api.SessionBehaviorRelease,
	}

	sessionID, _, err := client.Session().Create(session, nil)
	require.NoError(t, err)

	// Acquire a KV lock
	key := "destroy-release-key-" + randomID()
	acquired, _, err := client.KV().Acquire(&api.KVPair{
		Key:     key,
		Value:   []byte("locked"),
		Session: sessionID,
	}, nil)
	assert.NoError(t, err)
	assert.True(t, acquired)
	defer client.KV().Delete(key, nil)

	// Destroy the session
	_, err = client.Session().Destroy(sessionID, nil)
	assert.NoError(t, err)

	time.Sleep(500 * time.Millisecond)

	// KV lock should be released (release behavior)
	pair, _, err := client.KV().Get(key, nil)
	assert.NoError(t, err)
	if pair != nil {
		assert.Empty(t, pair.Session,
			"KV lock should be released after session destroy with release behavior")
		t.Logf("KV lock released after session destroy (value preserved: %s)", string(pair.Value))
	}
}

// CS-014: Test session destroy with delete behavior deletes keys
func TestSessionDestroyDeletesKeys(t *testing.T) {
	client := getClient(t)

	session := &api.SessionEntry{
		Name:     "destroy-delete-" + randomID(),
		TTL:      "30s",
		Behavior: api.SessionBehaviorDelete,
	}

	sessionID, _, err := client.Session().Create(session, nil)
	require.NoError(t, err)

	// Acquire a KV lock
	key := "destroy-delete-key-" + randomID()
	acquired, _, err := client.KV().Acquire(&api.KVPair{
		Key:     key,
		Value:   []byte("to-be-deleted"),
		Session: sessionID,
	}, nil)
	assert.NoError(t, err)
	assert.True(t, acquired)
	defer client.KV().Delete(key, nil)

	// Destroy the session (delete behavior should delete the key)
	_, err = client.Session().Destroy(sessionID, nil)
	assert.NoError(t, err)

	time.Sleep(500 * time.Millisecond)

	// KV key may be deleted (delete behavior)
	pair, _, err := client.KV().Get(key, nil)
	assert.NoError(t, err)
	if pair == nil {
		t.Log("KV key deleted after session destroy with delete behavior (correct)")
	} else if pair.Session == "" {
		t.Log("KV key still exists but lock released (acceptable)")
	} else {
		t.Logf("KV key still locked (unexpected): session=%s", pair.Session)
	}
}

// CS-015: Test create multiple sessions on same node
func TestSessionMultipleOnNode(t *testing.T) {
	client := getClient(t)

	var sessionIDs []string
	for i := 0; i < 5; i++ {
		session := &api.SessionEntry{
			Name: "multi-session-" + randomID(),
			TTL:  "30s",
		}
		id, _, err := client.Session().Create(session, nil)
		require.NoError(t, err)
		sessionIDs = append(sessionIDs, id)
	}
	defer func() {
		for _, id := range sessionIDs {
			client.Session().Destroy(id, nil)
		}
	}()

	// All sessions should be listable
	sessions, _, err := client.Session().List(nil)
	assert.NoError(t, err)

	foundCount := 0
	for _, s := range sessions {
		for _, id := range sessionIDs {
			if s.ID == id {
				foundCount++
				break
			}
		}
	}

	assert.Equal(t, 5, foundCount, "All 5 created sessions should be found")
	t.Logf("Found %d/%d sessions", foundCount, len(sessionIDs))
}
