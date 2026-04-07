package tests

import (
	"testing"
	"time"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// ==================== P0: Critical Tests ====================

// CS-001: Test session create with default validation
// Consul defaults: LockDelay=15s, Behavior="release", NodeChecks=["serfHealth"], TTL=""
func TestSessionCreateDefaults(t *testing.T) {
	client := getClient(t)

	// Create session with ONLY a name — all other fields should be defaulted
	sessionID, wm, err := client.Session().Create(&api.SessionEntry{
		Name: "test-defaults-" + randomID(),
	}, nil)
	require.NoError(t, err, "Session create should succeed")
	require.NotEmpty(t, sessionID, "Session ID must not be empty")
	assert.Greater(t, wm.RequestTime, time.Duration(0))
	defer client.Session().Destroy(sessionID, nil)

	// Get info and verify ALL defaults
	info, meta, err := client.Session().Info(sessionID, nil)
	require.NoError(t, err)
	require.NotNil(t, info, "Session info must not be nil")

	// ID
	assert.Equal(t, sessionID, info.ID, "Session ID must match")

	// LockDelay default = 15s
	assert.Equal(t, 15*time.Second, info.LockDelay,
		"Default LockDelay must be 15s (Consul default)")

	// Behavior default = "release"
	assert.Equal(t, api.SessionBehaviorRelease, info.Behavior,
		"Default Behavior must be 'release' (Consul default)")

	// TTL default = "" (persistent session, no auto-expiry)
	assert.Empty(t, info.TTL,
		"Default TTL must be empty (persistent session, Consul default)")

	// NodeChecks default = ["serfHealth"]
	assert.Equal(t, []string{"serfHealth"}, info.NodeChecks,
		"Default NodeChecks must be ['serfHealth'] (Consul default)")

	// Node should be set
	assert.NotEmpty(t, info.Node, "Node must be set")

	// CreateIndex must be positive
	assert.Greater(t, info.CreateIndex, uint64(0))

	// QueryMeta
	assert.Greater(t, meta.LastIndex, uint64(0))
	assert.True(t, meta.KnownLeader)
}

// CS-002: Test session create with explicit TTL
func TestSessionCreateWithTTL(t *testing.T) {
	client := getClient(t)

	sessionID, _, err := client.Session().Create(&api.SessionEntry{
		Name: "ttl-session-" + randomID(),
		TTL:  "30s",
	}, nil)
	require.NoError(t, err)
	defer client.Session().Destroy(sessionID, nil)

	info, _, err := client.Session().Info(sessionID, nil)
	require.NoError(t, err)
	require.NotNil(t, info)

	// TTL should be set (Consul normalizes to "30s")
	assert.NotEmpty(t, info.TTL, "TTL must be set when explicitly provided")
	assert.Contains(t, info.TTL, "30", "TTL must contain '30'")
}

// CS-003: Test destroy session
func TestSessionDestroy(t *testing.T) {
	client := getClient(t)

	sessionID, _, err := client.Session().Create(&api.SessionEntry{
		Name: "destroy-" + randomID(),
		TTL:  "30s",
	}, nil)
	require.NoError(t, err)

	// Destroy
	_, err = client.Session().Destroy(sessionID, nil)
	require.NoError(t, err, "Session destroy should succeed")

	// Verify gone
	info, _, err := client.Session().Info(sessionID, nil)
	require.NoError(t, err)
	assert.Nil(t, info, "Destroyed session must return nil from Info()")
}

// CS-004: Test renew session
func TestSessionRenew(t *testing.T) {
	client := getClient(t)

	sessionID, _, err := client.Session().Create(&api.SessionEntry{
		Name: "renew-" + randomID(),
		TTL:  "15s",
	}, nil)
	require.NoError(t, err)
	defer client.Session().Destroy(sessionID, nil)

	// Renew
	entry, _, err := client.Session().Renew(sessionID, nil)
	require.NoError(t, err, "Session renew should succeed")
	require.NotNil(t, entry, "Renewed session must not be nil")
	assert.Equal(t, sessionID, entry.ID, "Renewed session ID must match")
}

// CS-005: Test renew destroyed session returns nil
func TestSessionRenewDestroyed(t *testing.T) {
	client := getClient(t)

	sessionID, _, err := client.Session().Create(&api.SessionEntry{
		Name: "renew-destroyed-" + randomID(),
		TTL:  "30s",
	}, nil)
	require.NoError(t, err)

	_, err = client.Session().Destroy(sessionID, nil)
	require.NoError(t, err)

	// Renew of destroyed session should return nil entry (not error)
	entry, _, err := client.Session().Renew(sessionID, nil)
	// Consul returns nil entry, some implementations may return error
	if err == nil {
		assert.Nil(t, entry,
			"Renew of destroyed session must return nil entry")
	}
}

// ==================== P1: Important Tests ====================

// CS-006: Test list sessions
func TestSessionList(t *testing.T) {
	client := getClient(t)

	// Create 3 sessions
	var ids []string
	for i := 0; i < 3; i++ {
		id, _, err := client.Session().Create(&api.SessionEntry{
			Name: "list-" + randomID(),
			TTL:  "30s",
		}, nil)
		require.NoError(t, err)
		ids = append(ids, id)
	}
	defer func() {
		for _, id := range ids {
			client.Session().Destroy(id, nil)
		}
	}()

	// List
	sessions, meta, err := client.Session().List(nil)
	require.NoError(t, err, "Session list should succeed")
	assert.GreaterOrEqual(t, len(sessions), 3, "Should have at least 3 sessions")
	assert.Greater(t, meta.LastIndex, uint64(0))

	// Verify our sessions are in the list
	foundCount := 0
	for _, s := range sessions {
		for _, id := range ids {
			if s.ID == id {
				foundCount++
				// Each session should have valid fields
				assert.NotEmpty(t, s.Node, "Session node must be set")
				assert.Greater(t, s.CreateIndex, uint64(0))
				break
			}
		}
	}
	assert.Equal(t, 3, foundCount, "All 3 created sessions must be found")
}

// CS-007: Test session with custom lock delay
func TestSessionLockDelay(t *testing.T) {
	client := getClient(t)

	sessionID, _, err := client.Session().Create(&api.SessionEntry{
		Name:      "lockdelay-" + randomID(),
		TTL:       "30s",
		LockDelay: 5 * time.Second,
	}, nil)
	require.NoError(t, err)
	defer client.Session().Destroy(sessionID, nil)

	info, _, err := client.Session().Info(sessionID, nil)
	require.NoError(t, err)
	require.NotNil(t, info)

	assert.Equal(t, 5*time.Second, info.LockDelay,
		"Custom LockDelay must be preserved")
}

// CS-008: Test session with delete behavior
func TestSessionDeleteBehavior(t *testing.T) {
	client := getClient(t)

	sessionID, _, err := client.Session().Create(&api.SessionEntry{
		Name:     "delete-behavior-" + randomID(),
		TTL:      "30s",
		Behavior: api.SessionBehaviorDelete,
	}, nil)
	require.NoError(t, err)
	defer client.Session().Destroy(sessionID, nil)

	info, _, err := client.Session().Info(sessionID, nil)
	require.NoError(t, err)
	require.NotNil(t, info)
	assert.Equal(t, api.SessionBehaviorDelete, info.Behavior,
		"Behavior must be 'delete'")
}

// CS-009: Test session with release behavior
func TestSessionReleaseBehavior(t *testing.T) {
	client := getClient(t)

	sessionID, _, err := client.Session().Create(&api.SessionEntry{
		Name:     "release-behavior-" + randomID(),
		TTL:      "30s",
		Behavior: api.SessionBehaviorRelease,
	}, nil)
	require.NoError(t, err)
	defer client.Session().Destroy(sessionID, nil)

	info, _, err := client.Session().Info(sessionID, nil)
	require.NoError(t, err)
	require.NotNil(t, info)
	assert.Equal(t, api.SessionBehaviorRelease, info.Behavior,
		"Behavior must be 'release'")
}

// CS-010: Test session KV lock — destroy with release behavior preserves key
func TestSessionDestroyReleasesLocks(t *testing.T) {
	client := getClient(t)

	sessionID, _, err := client.Session().Create(&api.SessionEntry{
		Name:     "destroy-release-" + randomID(),
		TTL:      "30s",
		Behavior: api.SessionBehaviorRelease,
	}, nil)
	require.NoError(t, err)

	// Acquire KV lock
	key := "session-release-" + randomID()
	acquired, _, err := client.KV().Acquire(&api.KVPair{
		Key:     key,
		Value:   []byte("locked-value"),
		Session: sessionID,
	}, nil)
	require.NoError(t, err)
	require.True(t, acquired)
	defer client.KV().Delete(key, nil)

	// Verify locked
	pair, _, err := client.KV().Get(key, nil)
	require.NoError(t, err)
	require.NotNil(t, pair)
	assert.Equal(t, sessionID, pair.Session)

	// Destroy session — release behavior should release lock but preserve key
	_, err = client.Session().Destroy(sessionID, nil)
	require.NoError(t, err)

	time.Sleep(500 * time.Millisecond)

	// Key should still exist but lock should be released
	pair, _, err = client.KV().Get(key, nil)
	require.NoError(t, err)
	require.NotNil(t, pair,
		"Key must still exist after session destroy with release behavior")
	assert.Empty(t, pair.Session,
		"Lock must be released after session destroy")
	assert.Equal(t, []byte("locked-value"), pair.Value,
		"Value must be preserved after session destroy with release behavior")
}

// CS-011: Test session KV lock — destroy with delete behavior deletes key
func TestSessionDestroyDeletesKeys(t *testing.T) {
	client := getClient(t)

	sessionID, _, err := client.Session().Create(&api.SessionEntry{
		Name:     "destroy-delete-" + randomID(),
		TTL:      "30s",
		Behavior: api.SessionBehaviorDelete,
	}, nil)
	require.NoError(t, err)

	// Acquire KV lock
	key := "session-delete-" + randomID()
	acquired, _, err := client.KV().Acquire(&api.KVPair{
		Key:     key,
		Value:   []byte("to-be-deleted"),
		Session: sessionID,
	}, nil)
	require.NoError(t, err)
	require.True(t, acquired)
	defer client.KV().Delete(key, nil)

	// Destroy session — delete behavior should delete the key
	_, err = client.Session().Destroy(sessionID, nil)
	require.NoError(t, err)

	time.Sleep(500 * time.Millisecond)

	// Key should be deleted
	pair, _, err := client.KV().Get(key, nil)
	require.NoError(t, err)
	assert.Nil(t, pair,
		"Key must be deleted after session destroy with delete behavior")
}

// CS-012: Test node sessions
func TestSessionNode(t *testing.T) {
	client := getClient(t)

	// Get node name
	self, err := client.Agent().Self()
	require.NoError(t, err)
	nodeName := self["Config"]["NodeName"].(string)
	require.NotEmpty(t, nodeName)

	// Create session
	sessionID, _, err := client.Session().Create(&api.SessionEntry{
		Name: "node-session-" + randomID(),
		TTL:  "30s",
	}, nil)
	require.NoError(t, err)
	defer client.Session().Destroy(sessionID, nil)

	// List sessions for this node
	sessions, meta, err := client.Session().Node(nodeName, nil)
	require.NoError(t, err, "Session.Node should succeed")
	assert.Greater(t, meta.LastIndex, uint64(0))

	// Our session should be in the list
	found := false
	for _, s := range sessions {
		if s.ID == sessionID {
			found = true
			assert.Equal(t, nodeName, s.Node,
				"Session node must match queried node")
			break
		}
	}
	assert.True(t, found, "Created session must be found in node session list")
}

// CS-013: Test multiple sessions on same node
func TestSessionMultipleOnNode(t *testing.T) {
	client := getClient(t)

	var ids []string
	for i := 0; i < 5; i++ {
		id, _, err := client.Session().Create(&api.SessionEntry{
			Name: "multi-" + randomID(),
			TTL:  "30s",
		}, nil)
		require.NoError(t, err)
		ids = append(ids, id)
	}
	defer func() {
		for _, id := range ids {
			client.Session().Destroy(id, nil)
		}
	}()

	sessions, _, err := client.Session().List(nil)
	require.NoError(t, err)

	foundCount := 0
	for _, s := range sessions {
		for _, id := range ids {
			if s.ID == id {
				foundCount++
				break
			}
		}
	}
	assert.Equal(t, 5, foundCount,
		"All 5 created sessions must be found in list")
}

// CS-014: Test CreateNoChecks
func TestSessionCreateNoChecks(t *testing.T) {
	client := getClient(t)

	sessionID, _, err := client.Session().CreateNoChecks(&api.SessionEntry{
		Name: "no-checks-" + randomID(),
		TTL:  "30s",
	}, nil)
	require.NoError(t, err, "CreateNoChecks should succeed")
	require.NotEmpty(t, sessionID)
	defer client.Session().Destroy(sessionID, nil)

	info, _, err := client.Session().Info(sessionID, nil)
	require.NoError(t, err)
	require.NotNil(t, info)
	assert.Equal(t, sessionID, info.ID)
}

// CS-015: Test periodic renewal
func TestSessionRenewPeriodic(t *testing.T) {
	client := getClient(t)

	sessionID, _, err := client.Session().Create(&api.SessionEntry{
		Name: "periodic-" + randomID(),
		TTL:  "15s",
	}, nil)
	require.NoError(t, err)
	defer client.Session().Destroy(sessionID, nil)

	doneCh := make(chan struct{})
	errCh := make(chan error, 1)

	go func() {
		errCh <- client.Session().RenewPeriodic("15s", sessionID, nil, doneCh)
	}()

	// Let it renew once
	time.Sleep(2 * time.Second)

	// Session should still be alive
	info, _, err := client.Session().Info(sessionID, nil)
	require.NoError(t, err)
	assert.NotNil(t, info,
		"Session must still be alive during periodic renewal")

	close(doneCh)
	err = <-errCh
	assert.NoError(t, err)

	// After close, RenewPeriodic destroys the session
	info, _, err = client.Session().Info(sessionID, nil)
	require.NoError(t, err)
	assert.Nil(t, info,
		"Session must be destroyed after RenewPeriodic stops")
}

// ==================== Health-Check → Session Invalidation ====================

// CS-016: Test session auto-invalidation when linked check becomes Critical.
// Consul core behavior: sessions are automatically destroyed when their associated
// health checks fail. This is the safety mechanism for distributed locking —
// if a node holding a lock becomes unhealthy, its sessions are invalidated and
// locks are released so other nodes can acquire them.
func TestSessionInvalidatedOnCheckCritical(t *testing.T) {
	client := getClient(t)

	serviceID := "cs016-session-inval-" + randomID()
	checkID := "service:" + serviceID

	// Step 1: Register a service with a TTL check (starts passing)
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceID,
		Name: serviceID,
		Port: 8080,
		Check: &api.AgentServiceCheck{
			CheckID: checkID,
			TTL:     "30s",
			Status:  "passing",
		},
	})
	require.NoError(t, err, "Service registration should succeed")
	defer client.Agent().ServiceDeregister(serviceID)

	time.Sleep(500 * time.Millisecond)

	// Step 2: Create a session linked to this service check
	sessionID, _, err := client.Session().Create(&api.SessionEntry{
		Name:          "check-linked-" + randomID(),
		TTL:           "60s",
		NodeChecks:    []string{},                // no node checks
		ServiceChecks: []api.ServiceCheck{{ID: checkID}},
	}, nil)
	require.NoError(t, err, "Session create should succeed")
	require.NotEmpty(t, sessionID)

	// Step 3: Acquire a KV lock with this session
	kvKey := "test/session-inval/" + randomID()
	acquired, _, err := client.KV().Acquire(&api.KVPair{
		Key:     kvKey,
		Value:   []byte("locked-by-session"),
		Session: sessionID,
	}, nil)
	require.NoError(t, err)
	require.True(t, acquired, "KV lock should be acquired")
	defer client.KV().Delete(kvKey, nil)

	// Verify session exists and lock is held
	info, _, err := client.Session().Info(sessionID, nil)
	require.NoError(t, err)
	require.NotNil(t, info, "Session should exist before check failure")

	pair, _, err := client.KV().Get(kvKey, nil)
	require.NoError(t, err)
	require.NotNil(t, pair)
	assert.Equal(t, sessionID, pair.Session, "KV should be locked by session")

	// Step 4: Fail the check (mark as Critical)
	err = client.Agent().FailTTL(checkID, "node crashed")
	require.NoError(t, err, "FailTTL should succeed")

	// Wait for session invalidation to propagate
	time.Sleep(2 * time.Second)

	// Step 5: Verify session was automatically invalidated
	info, _, err = client.Session().Info(sessionID, nil)
	require.NoError(t, err)
	assert.Nil(t, info,
		"Session must be automatically destroyed when linked check becomes Critical")

	// Step 6: Verify KV lock was released (release behavior is default)
	pair, _, err = client.KV().Get(kvKey, nil)
	require.NoError(t, err)
	if pair != nil {
		assert.Empty(t, pair.Session,
			"KV lock must be released after session invalidation")
	}
}

// CS-017: Test session with serfHealth — session should survive while node is healthy.
// This is the default session behavior: NodeChecks=["serfHealth"].
func TestSessionSurvivesWithHealthySerfCheck(t *testing.T) {
	client := getClient(t)

	// Create session with default checks (serfHealth)
	sessionID, _, err := client.Session().Create(&api.SessionEntry{
		Name: "serf-healthy-" + randomID(),
		TTL:  "30s",
	}, nil)
	require.NoError(t, err)
	defer client.Session().Destroy(sessionID, nil)

	// Session should exist (serfHealth is passing)
	info, _, err := client.Session().Info(sessionID, nil)
	require.NoError(t, err)
	require.NotNil(t, info, "Session should exist while serfHealth is passing")
	assert.Equal(t, sessionID, info.ID)

	// serfHealth should be passing
	checks, _, err := client.Health().State("passing", nil)
	require.NoError(t, err)
	found := false
	for _, c := range checks {
		if c.CheckID == "serfHealth" {
			found = true
			assert.Equal(t, api.HealthPassing, c.Status)
			break
		}
	}
	assert.True(t, found, "serfHealth check should exist and be passing")
}

// CS-018: Test session invalidation with delete behavior — KV keys should be deleted.
func TestSessionInvalidatedOnCheckCriticalDeleteBehavior(t *testing.T) {
	client := getClient(t)

	serviceID := "cs018-del-inval-" + randomID()
	checkID := "service:" + serviceID

	// Register service with TTL check
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceID,
		Name: serviceID,
		Port: 8081,
		Check: &api.AgentServiceCheck{
			CheckID: checkID,
			TTL:     "30s",
			Status:  "passing",
		},
	})
	require.NoError(t, err)
	defer client.Agent().ServiceDeregister(serviceID)

	time.Sleep(500 * time.Millisecond)

	// Create session with DELETE behavior linked to service check
	sessionID, _, err := client.Session().Create(&api.SessionEntry{
		Name:          "delete-inval-" + randomID(),
		TTL:           "60s",
		Behavior:      api.SessionBehaviorDelete,
		NodeChecks:    []string{},
		ServiceChecks: []api.ServiceCheck{{ID: checkID}},
	}, nil)
	require.NoError(t, err)
	require.NotEmpty(t, sessionID)

	// Acquire KV lock
	kvKey := "test/session-del-inval/" + randomID()
	acquired, _, err := client.KV().Acquire(&api.KVPair{
		Key:     kvKey,
		Value:   []byte("will-be-deleted"),
		Session: sessionID,
	}, nil)
	require.NoError(t, err)
	require.True(t, acquired)
	defer client.KV().Delete(kvKey, nil)

	// Fail the check
	err = client.Agent().FailTTL(checkID, "check failed")
	require.NoError(t, err)

	time.Sleep(2 * time.Second)

	// Session should be invalidated
	info, _, err := client.Session().Info(sessionID, nil)
	require.NoError(t, err)
	assert.Nil(t, info,
		"Session with delete behavior must be invalidated on check Critical")

	// KV key should be DELETED (not just released)
	pair, _, err := client.KV().Get(kvKey, nil)
	require.NoError(t, err)
	assert.Nil(t, pair,
		"KV key must be deleted when session with delete behavior is invalidated")
}
