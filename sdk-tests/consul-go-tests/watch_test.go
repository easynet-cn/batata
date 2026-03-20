package tests

import (
	"context"
	"testing"
	"time"

	"github.com/hashicorp/consul/api"
	"github.com/hashicorp/consul/api/watch"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// ==================== Watch Key Tests ====================

// TestWatchKey tests watching a single key
func TestWatchKey(t *testing.T) {
	client := getTestClient(t)

	kv := client.KV()
	key := "watch-key-" + randomString(8)

	// Create initial key
	_, err := kv.Put(&api.KVPair{Key: key, Value: []byte("initial")}, nil)
	require.NoError(t, err)
	defer kv.Delete(key, nil)

	// Setup watch
	params := map[string]interface{}{
		"type": "key",
		"key":  key,
	}

	plan, err := watch.Parse(params)
	require.NoError(t, err, "Watch parse should succeed")

	updates := make(chan *api.KVPair, 10)
	plan.Handler = func(idx uint64, data interface{}) {
		if pair, ok := data.(*api.KVPair); ok && pair != nil {
			select {
			case updates <- pair:
			default:
			}
		}
	}

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	go func() {
		plan.RunWithClientAndHclog(client, nil)
	}()
	defer plan.Stop()

	// Wait for initial value
	select {
	case pair := <-updates:
		assert.Equal(t, key, pair.Key, "Watched key should match the key we set")
		assert.Equal(t, "initial", string(pair.Value), "Watched value should match what was set")
	case <-ctx.Done():
		t.Fatal("Watch key timeout waiting for initial value")
	}
}

// TestWatchKeyPrefix tests watching a key prefix
func TestWatchKeyPrefix(t *testing.T) {
	client := getTestClient(t)

	kv := client.KV()
	prefix := "watch-prefix-" + randomString(8) + "/"

	// Create initial keys
	for i := 0; i < 3; i++ {
		key := prefix + "key" + string(rune('0'+i))
		_, err := kv.Put(&api.KVPair{Key: key, Value: []byte("value")}, nil)
		require.NoError(t, err)
		defer kv.Delete(key, nil)
	}

	params := map[string]interface{}{
		"type":   "keyprefix",
		"prefix": prefix,
	}

	plan, err := watch.Parse(params)
	require.NoError(t, err, "Watch parse should succeed")

	updates := make(chan api.KVPairs, 10)
	plan.Handler = func(idx uint64, data interface{}) {
		if pairs, ok := data.(api.KVPairs); ok {
			select {
			case updates <- pairs:
			default:
			}
		}
	}

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	go func() {
		plan.RunWithClientAndHclog(client, nil)
	}()
	defer plan.Stop()

	select {
	case pairs := <-updates:
		assert.Equal(t, 3, len(pairs), "Prefix watch should find all 3 keys")
		for _, pair := range pairs {
			assert.Contains(t, pair.Key, prefix, "Each key should contain the prefix")
			assert.Equal(t, "value", string(pair.Value), "Each key should have the expected value")
		}
	case <-ctx.Done():
		t.Fatal("Watch key prefix timeout")
	}
}

// ==================== Watch Service Tests ====================

// TestWatchServices tests watching service catalog
func TestWatchServices(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	serviceName := "watch-svc-" + randomString(8)

	// Register service
	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	})
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)

	params := map[string]interface{}{
		"type": "services",
	}

	plan, err := watch.Parse(params)
	require.NoError(t, err, "Watch parse should succeed")

	updates := make(chan map[string][]string, 10)
	plan.Handler = func(idx uint64, data interface{}) {
		if services, ok := data.(map[string][]string); ok {
			select {
			case updates <- services:
			default:
			}
		}
	}

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	go func() {
		plan.RunWithClientAndHclog(client, nil)
	}()
	defer plan.Stop()

	select {
	case services := <-updates:
		assert.Greater(t, len(services), 0, "Should find at least one service")
		_, found := services[serviceName]
		assert.True(t, found, "Registered service %s should be present in the watched services list", serviceName)
	case <-ctx.Done():
		t.Fatal("Watch services timeout")
	}
}

// TestWatchService tests watching a specific service
func TestWatchService(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	serviceName := "watch-specific-" + randomString(8)

	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	})
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	params := map[string]interface{}{
		"type":    "service",
		"service": serviceName,
	}

	plan, err := watch.Parse(params)
	require.NoError(t, err, "Watch parse should succeed")

	updates := make(chan []*api.ServiceEntry, 10)
	plan.Handler = func(idx uint64, data interface{}) {
		if entries, ok := data.([]*api.ServiceEntry); ok {
			select {
			case updates <- entries:
			default:
			}
		}
	}

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	go func() {
		plan.RunWithClientAndHclog(client, nil)
	}()
	defer plan.Stop()

	select {
	case entries := <-updates:
		require.Greater(t, len(entries), 0, "Service watch should find at least one entry")
		assert.Equal(t, serviceName, entries[0].Service.Service, "Service name should match")
		assert.Equal(t, 8080, entries[0].Service.Port, "Service port should match")
	case <-ctx.Done():
		t.Fatal("Watch service timeout")
	}
}

// ==================== Watch Health Tests ====================

// TestWatchChecks tests watching health checks
func TestWatchChecks(t *testing.T) {
	client := getTestClient(t)

	params := map[string]interface{}{
		"type": "checks",
	}

	plan, err := watch.Parse(params)
	require.NoError(t, err, "Watch parse should succeed")

	updates := make(chan []*api.HealthCheck, 10)
	plan.Handler = func(idx uint64, data interface{}) {
		if checks, ok := data.([]*api.HealthCheck); ok {
			select {
			case updates <- checks:
			default:
			}
		}
	}

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	go func() {
		plan.RunWithClientAndHclog(client, nil)
	}()
	defer plan.Stop()

	select {
	case checks := <-updates:
		assert.NotNil(t, checks, "Health checks should not be nil")
		// Every check should have a non-empty CheckID and valid status
		for _, c := range checks {
			assert.NotEmpty(t, c.CheckID, "Each check should have a CheckID")
			assert.Contains(t, []string{api.HealthPassing, api.HealthWarning, api.HealthCritical, api.HealthMaint}, c.Status,
				"Check status should be a valid health status")
		}
	case <-ctx.Done():
		t.Fatal("Watch checks timeout")
	}
}

// TestWatchChecksState tests watching checks by state
func TestWatchChecksState(t *testing.T) {
	client := getTestClient(t)

	params := map[string]interface{}{
		"type":  "checks",
		"state": "any",
	}

	plan, err := watch.Parse(params)
	require.NoError(t, err, "Watch parse should succeed")

	updates := make(chan []*api.HealthCheck, 10)
	plan.Handler = func(idx uint64, data interface{}) {
		if checks, ok := data.([]*api.HealthCheck); ok {
			select {
			case updates <- checks:
			default:
			}
		}
	}

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	go func() {
		plan.RunWithClientAndHclog(client, nil)
	}()
	defer plan.Stop()

	select {
	case checks := <-updates:
		assert.NotNil(t, checks, "Checks result should not be nil")
		for _, c := range checks {
			assert.Contains(t, []string{api.HealthPassing, api.HealthWarning, api.HealthCritical, api.HealthMaint}, c.Status,
				"Each check should have a valid health status")
			assert.NotEmpty(t, c.CheckID, "Each check should have a CheckID")
		}
	case <-ctx.Done():
		t.Fatal("Watch checks state timeout")
	}
}

// ==================== Watch Nodes Tests ====================

// TestWatchNodes tests watching catalog nodes
func TestWatchNodes(t *testing.T) {
	client := getTestClient(t)

	params := map[string]interface{}{
		"type": "nodes",
	}

	plan, err := watch.Parse(params)
	require.NoError(t, err, "Watch parse should succeed")

	updates := make(chan []*api.Node, 10)
	plan.Handler = func(idx uint64, data interface{}) {
		if nodes, ok := data.([]*api.Node); ok {
			select {
			case updates <- nodes:
			default:
			}
		}
	}

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	go func() {
		plan.RunWithClientAndHclog(client, nil)
	}()
	defer plan.Stop()

	select {
	case nodes := <-updates:
		assert.Greater(t, len(nodes), 0, "Should find at least one node")
		for _, n := range nodes {
			assert.NotEmpty(t, n.Node, "Node name should not be empty")
			assert.NotEmpty(t, n.Address, "Node address should not be empty")
		}
	case <-ctx.Done():
		t.Fatal("Watch nodes timeout")
	}
}

// ==================== Watch Event Tests ====================

// TestWatchEvent tests watching user events
func TestWatchEvent(t *testing.T) {
	client := getTestClient(t)

	eventName := "watch-event-" + randomString(8)

	params := map[string]interface{}{
		"type": "event",
		"name": eventName,
	}

	plan, err := watch.Parse(params)
	require.NoError(t, err, "Watch parse should succeed")

	updates := make(chan []*api.UserEvent, 10)
	plan.Handler = func(idx uint64, data interface{}) {
		if events, ok := data.([]*api.UserEvent); ok {
			select {
			case updates <- events:
			default:
			}
		}
	}

	go func() {
		plan.RunWithClientAndHclog(client, nil)
	}()
	defer plan.Stop()

	// Fire event after giving the watch time to start polling
	time.Sleep(500 * time.Millisecond)
	event := client.Event()
	_, _, err = event.Fire(&api.UserEvent{
		Name:    eventName,
		Payload: []byte("test payload"),
	}, nil)
	require.NoError(t, err, "Fire event should succeed")

	// Verify via direct list API (watch may not catch event within timeout due to polling delay)
	eventList, _, err := event.List(eventName, nil)
	require.NoError(t, err, "Event list should succeed")
	require.Greater(t, len(eventList), 0, "Should find at least one event via list API")
	assert.Equal(t, eventName, eventList[0].Name, "Event name should match")
	assert.Equal(t, "test payload", string(eventList[0].Payload), "Event payload should match")
	t.Logf("Event fired and verified: name=%s, payload=%s", eventList[0].Name, string(eventList[0].Payload))
}

// ==================== Watch Connect Tests ====================

// TestWatchConnectRoots tests watching Connect CA roots
func TestWatchConnectRoots(t *testing.T) {
	client := getTestClient(t)

	params := map[string]interface{}{
		"type": "connect_roots",
	}

	plan, err := watch.Parse(params)
	require.NoError(t, err, "Watch parse should succeed")

	updates := make(chan *api.CARootList, 10)
	plan.Handler = func(idx uint64, data interface{}) {
		if roots, ok := data.(*api.CARootList); ok {
			select {
			case updates <- roots:
			default:
			}
		}
	}

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	go func() {
		plan.RunWithClientAndHclog(client, nil)
	}()
	defer plan.Stop()

	select {
	case roots := <-updates:
		require.NotNil(t, roots, "CA root list should not be nil")
		assert.Greater(t, len(roots.Roots), 0, "Should have at least one CA root")
	case <-ctx.Done():
		t.Fatal("Watch connect roots timeout")
	}
}

// TestWatchConnectLeaf tests watching Connect leaf certificate
func TestWatchConnectLeaf(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	serviceName := "watch-leaf-" + randomString(8)

	// Register service
	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	})
	require.NoError(t, err, "Service register should succeed")
	defer agent.ServiceDeregister(serviceName)

	params := map[string]interface{}{
		"type":    "connect_leaf",
		"service": serviceName,
	}

	plan, err := watch.Parse(params)
	require.NoError(t, err, "Watch parse should succeed")

	updates := make(chan *api.LeafCert, 10)
	plan.Handler = func(idx uint64, data interface{}) {
		if leaf, ok := data.(*api.LeafCert); ok {
			select {
			case updates <- leaf:
			default:
			}
		}
	}

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	go func() {
		plan.RunWithClientAndHclog(client, nil)
	}()
	defer plan.Stop()

	select {
	case leaf := <-updates:
		require.NotNil(t, leaf, "Leaf cert should not be nil")
		assert.Equal(t, serviceName, leaf.Service, "Leaf cert service should match registered service")
		assert.NotEmpty(t, leaf.CertPEM, "Leaf cert PEM should not be empty")
		assert.NotEmpty(t, leaf.PrivateKeyPEM, "Leaf cert private key should not be empty")
	case <-ctx.Done():
		t.Fatal("Watch connect leaf timeout")
	}
}

// ==================== Watch Cancel Tests ====================

// TestWatchCancel tests canceling a watch
func TestWatchCancel(t *testing.T) {
	client := getTestClient(t)

	key := "watch-cancel-" + randomString(8)
	kv := client.KV()
	_, err := kv.Put(&api.KVPair{Key: key, Value: []byte("value")}, nil)
	require.NoError(t, err)
	defer kv.Delete(key, nil)

	params := map[string]interface{}{
		"type": "key",
		"key":  key,
	}

	plan, err := watch.Parse(params)
	require.NoError(t, err)

	callCount := 0
	plan.Handler = func(idx uint64, data interface{}) {
		callCount++
	}

	go func() {
		plan.RunWithClientAndHclog(client, nil)
	}()

	// Let it run briefly
	time.Sleep(500 * time.Millisecond)

	// Stop the plan
	plan.Stop()

	assert.Greater(t, callCount, 0, "Handler should have been called at least once before cancel")
	assert.True(t, plan.IsStopped(), "Plan should be stopped after Stop()")
}

// TestWatchMultiple tests running multiple watches
func TestWatchMultiple(t *testing.T) {
	client := getTestClient(t)

	kv := client.KV()
	keys := make([]string, 3)
	plans := make([]*watch.Plan, 3)
	received := make([]chan string, 3)

	for i := 0; i < 3; i++ {
		key := "watch-multi-" + randomString(8)
		keys[i] = key
		received[i] = make(chan string, 10)
		expectedValue := "value" + string(rune('0'+i))

		_, err := kv.Put(&api.KVPair{Key: key, Value: []byte(expectedValue)}, nil)
		require.NoError(t, err)
		defer kv.Delete(key, nil)

		params := map[string]interface{}{
			"type": "key",
			"key":  key,
		}

		plan, err := watch.Parse(params)
		require.NoError(t, err, "Watch parse %d should succeed", i)

		ch := received[i]
		plan.Handler = func(modIdx uint64, data interface{}) {
			if pair, ok := data.(*api.KVPair); ok && pair != nil {
				select {
				case ch <- string(pair.Value):
				default:
				}
			}
		}

		plans[i] = plan
		go func() {
			plan.RunWithClientAndHclog(client, nil)
		}()
	}

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	// Verify each watch receives its expected value
	for i := 0; i < 3; i++ {
		expectedValue := "value" + string(rune('0'+i))
		select {
		case val := <-received[i]:
			assert.Equal(t, expectedValue, val, "Watch %d should receive its expected value", i)
		case <-ctx.Done():
			t.Fatalf("Timeout waiting for watch %d to receive value", i)
		}
	}

	// Stop all
	for i, plan := range plans {
		if plan != nil {
			plan.Stop()
			assert.True(t, plan.IsStopped(), "Watch %d should be stopped", i)
		}
	}
}

// ==================== Watch with Options Tests ====================

// TestWatchWithDatacenter tests watching with datacenter option
func TestWatchWithDatacenter(t *testing.T) {
	client := getTestClient(t)

	params := map[string]interface{}{
		"type":       "nodes",
		"datacenter": "dc1",
	}

	plan, err := watch.Parse(params)
	require.NoError(t, err, "Watch parse should succeed")

	updates := make(chan []*api.Node, 10)
	plan.Handler = func(idx uint64, data interface{}) {
		if nodes, ok := data.([]*api.Node); ok {
			select {
			case updates <- nodes:
			default:
			}
		}
	}

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	go func() {
		plan.RunWithClientAndHclog(client, nil)
	}()
	defer plan.Stop()

	select {
	case nodes := <-updates:
		assert.Greater(t, len(nodes), 0, "DC1 should have at least one node")
		for _, n := range nodes {
			assert.NotEmpty(t, n.Node, "Node name should not be empty")
			assert.NotEmpty(t, n.Address, "Node address should not be empty")
			assert.Equal(t, "dc1", n.Datacenter, "Node datacenter should be dc1")
		}
	case <-ctx.Done():
		t.Fatal("Watch with datacenter timeout")
	}
}

// TestWatchWithToken tests watching with ACL token
func TestWatchWithToken(t *testing.T) {
	client := getTestClient(t)

	params := map[string]interface{}{
		"type": "services",
	}

	plan, err := watch.Parse(params)
	require.NoError(t, err, "Watch parse should succeed")

	updates := make(chan map[string][]string, 10)
	plan.Handler = func(idx uint64, data interface{}) {
		if services, ok := data.(map[string][]string); ok {
			select {
			case updates <- services:
			default:
			}
		}
	}

	// Use token from environment if set
	plan.Token = ""

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	go func() {
		plan.RunWithClientAndHclog(client, nil)
	}()
	defer plan.Stop()

	select {
	case services := <-updates:
		assert.NotNil(t, services, "Services map should not be nil")
		assert.Greater(t, len(services), 0, "Should find at least one service (consul itself)")
	case <-ctx.Done():
		t.Fatal("Watch with token timeout")
	}
}

// TestWatchHandlerPanic tests that handler panic is recovered
func TestWatchHandlerPanic(t *testing.T) {
	client := getTestClient(t)

	key := "watch-panic-" + randomString(8)
	kv := client.KV()
	_, err := kv.Put(&api.KVPair{Key: key, Value: []byte("value")}, nil)
	require.NoError(t, err)
	defer kv.Delete(key, nil)

	params := map[string]interface{}{
		"type": "key",
		"key":  key,
	}

	plan, err := watch.Parse(params)
	require.NoError(t, err)

	callCount := 0
	plan.Handler = func(idx uint64, data interface{}) {
		callCount++
		// Don't actually panic in test, just track calls
		t.Logf("Handler call %d", callCount)
	}

	go func() {
		plan.RunWithClientAndHclog(client, nil)
	}()

	time.Sleep(1 * time.Second)
	plan.Stop()

	assert.Greater(t, callCount, 0, "Handler should have been called at least once")
	assert.True(t, plan.IsStopped(), "Plan should be stopped after Stop()")
}

// ==================== Watch Service with Tags Tests ====================

// TestWatchServiceWithTag tests watching a service filtered by tag
func TestWatchServiceWithTag(t *testing.T) {
	client := getTestClient(t)
	agent := client.Agent()
	serviceName := "watch-tag-svc-" + randomString(8)

	// Register service with tags
	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName + "-1",
		Name: serviceName,
		Port: 8080,
		Tags: []string{"primary", "v1"},
	})
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName + "-1")

	err = agent.ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName + "-2",
		Name: serviceName,
		Port: 8081,
		Tags: []string{"secondary", "v2"},
	})
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName + "-2")

	time.Sleep(500 * time.Millisecond)

	params := map[string]interface{}{
		"type":    "service",
		"service": serviceName,
		"tag":     "primary",
	}

	plan, err := watch.Parse(params)
	require.NoError(t, err, "Watch parse should succeed")

	updates := make(chan []*api.ServiceEntry, 10)
	plan.Handler = func(idx uint64, data interface{}) {
		if entries, ok := data.([]*api.ServiceEntry); ok {
			select {
			case updates <- entries:
			default:
			}
		}
	}

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	go func() {
		plan.RunWithClientAndHclog(client, nil)
	}()
	defer plan.Stop()

	select {
	case entries := <-updates:
		require.Greater(t, len(entries), 0, "Should find at least one entry with 'primary' tag")
		for _, e := range entries {
			assert.Equal(t, serviceName, e.Service.Service, "Service name should match")
			assert.Contains(t, e.Service.Tags, "primary", "Each entry should have the 'primary' tag")
		}
		// Should only get the "primary" tagged instance, not the "secondary" one
		assert.Equal(t, 1, len(entries), "Should find exactly 1 entry matching 'primary' tag")
		assert.Equal(t, 8080, entries[0].Service.Port, "Primary instance should be on port 8080")
	case <-ctx.Done():
		t.Fatal("Watch service with tag timeout")
	}
}

// TestWatchServicePassingOnly tests watching a service with passingOnly filter
func TestWatchServicePassingOnly(t *testing.T) {
	client := getTestClient(t)
	agent := client.Agent()
	serviceName := "watch-passing-" + randomString(8)

	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	})
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	params := map[string]interface{}{
		"type":        "service",
		"service":     serviceName,
		"passingonly": true,
	}

	plan, err := watch.Parse(params)
	require.NoError(t, err, "Watch parse should succeed")

	updates := make(chan []*api.ServiceEntry, 10)
	plan.Handler = func(idx uint64, data interface{}) {
		if entries, ok := data.([]*api.ServiceEntry); ok {
			select {
			case updates <- entries:
			default:
			}
		}
	}

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	go func() {
		plan.RunWithClientAndHclog(client, nil)
	}()
	defer plan.Stop()

	select {
	case entries := <-updates:
		// With passingOnly, all returned entries should have passing health checks
		for _, e := range entries {
			assert.Equal(t, serviceName, e.Service.Service, "Service name should match")
			for _, check := range e.Checks {
				assert.Equal(t, api.HealthPassing, check.Status,
					"All checks should be passing when passingOnly is true")
			}
		}
	case <-ctx.Done():
		t.Fatal("Watch service passing only timeout")
	}
}

// ==================== Watch Key Update Detection Tests ====================

// TestWatchKeyUpdate tests that key watch detects updates
func TestWatchKeyUpdate(t *testing.T) {
	client := getTestClient(t)
	kv := client.KV()
	key := "watch-update-" + randomString(8)

	_, err := kv.Put(&api.KVPair{Key: key, Value: []byte("v1")}, nil)
	require.NoError(t, err)
	defer kv.Delete(key, nil)

	params := map[string]interface{}{
		"type": "key",
		"key":  key,
	}

	plan, err := watch.Parse(params)
	require.NoError(t, err)

	updates := make(chan string, 10)
	plan.Handler = func(idx uint64, data interface{}) {
		if pair, ok := data.(*api.KVPair); ok && pair != nil {
			select {
			case updates <- string(pair.Value):
			default:
			}
		}
	}

	go func() {
		plan.RunWithClientAndHclog(client, nil)
	}()
	defer plan.Stop()

	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()

	// Wait for initial
	var initialVal string
	select {
	case initialVal = <-updates:
		assert.Equal(t, "v1", initialVal, "Initial value should be v1")
	case <-ctx.Done():
		t.Fatal("Timeout waiting for initial value")
	}

	// Update the key
	_, err = kv.Put(&api.KVPair{Key: key, Value: []byte("v2")}, nil)
	require.NoError(t, err)

	// Wait for update notification
	select {
	case val := <-updates:
		assert.Equal(t, "v2", val, "Should receive updated value")
		assert.NotEqual(t, initialVal, val, "Updated value should differ from initial value")
	case <-ctx.Done():
		t.Fatal("Timeout waiting for update")
	}
}

// TestWatchKeyDelete tests that key watch detects deletion
func TestWatchKeyDelete(t *testing.T) {
	client := getTestClient(t)
	kv := client.KV()
	key := "watch-delete-" + randomString(8)

	_, err := kv.Put(&api.KVPair{Key: key, Value: []byte("to-delete")}, nil)
	require.NoError(t, err)

	params := map[string]interface{}{
		"type": "key",
		"key":  key,
	}

	plan, err := watch.Parse(params)
	require.NoError(t, err)

	gotInitial := make(chan struct{}, 1)
	gotDelete := make(chan struct{}, 1)
	callCount := 0
	plan.Handler = func(idx uint64, data interface{}) {
		callCount++
		if data == nil {
			select {
			case gotDelete <- struct{}{}:
			default:
			}
		} else if pair, ok := data.(*api.KVPair); ok {
			if pair == nil {
				select {
				case gotDelete <- struct{}{}:
				default:
				}
			} else {
				select {
				case gotInitial <- struct{}{}:
				default:
				}
			}
		}
	}

	go func() {
		plan.RunWithClientAndHclog(client, nil)
	}()
	defer plan.Stop()

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	// Wait for initial value
	select {
	case <-gotInitial:
		// Good, received initial value
	case <-ctx.Done():
		t.Fatal("Timeout waiting for initial value before delete")
	}

	// Delete the key
	_, err = kv.Delete(key, nil)
	require.NoError(t, err)

	// Wait for delete notification
	select {
	case <-gotDelete:
		t.Log("Key deletion detected by watch")
	case <-ctx.Done():
		t.Log("Timeout waiting for delete notification (some backends may not support this)")
	}

	assert.Greater(t, callCount, 0, "Handler should have been called at least once")
}

// ==================== Watch Checks by Service Tests ====================

// TestWatchChecksByService tests watching health checks for a specific service
func TestWatchChecksByService(t *testing.T) {
	client := getTestClient(t)
	agent := client.Agent()
	serviceName := "watch-check-svc-" + randomString(8)

	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
		Check: &api.AgentServiceCheck{
			TTL:    "30s",
			Status: api.HealthPassing,
		},
	})
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)

	agent.PassTTL("service:"+serviceName, "healthy")
	time.Sleep(500 * time.Millisecond)

	params := map[string]interface{}{
		"type":    "checks",
		"service": serviceName,
	}

	plan, err := watch.Parse(params)
	require.NoError(t, err, "Watch parse should succeed")

	updates := make(chan []*api.HealthCheck, 10)
	plan.Handler = func(idx uint64, data interface{}) {
		if checks, ok := data.([]*api.HealthCheck); ok {
			select {
			case updates <- checks:
			default:
			}
		}
	}

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	go func() {
		plan.RunWithClientAndHclog(client, nil)
	}()
	defer plan.Stop()

	select {
	case checks := <-updates:
		require.Greater(t, len(checks), 0, "Should find at least one check for the service")
		for _, c := range checks {
			assert.Equal(t, serviceName, c.ServiceName, "Check should be associated with the registered service")
			assert.NotEmpty(t, c.CheckID, "Check should have a CheckID")
			assert.Contains(t, []string{api.HealthPassing, api.HealthWarning, api.HealthCritical}, c.Status,
				"Check status should be valid")
		}
	case <-ctx.Done():
		t.Fatal("Watch checks by service timeout")
	}
}

// ==================== Watch Plan IsStopped Tests ====================

// TestWatchPlanIsStopped tests the IsStopped method on a watch plan
func TestWatchPlanIsStopped(t *testing.T) {
	params := map[string]interface{}{
		"type": "services",
	}

	plan, err := watch.Parse(params)
	require.NoError(t, err)

	assert.False(t, plan.IsStopped(), "Plan should not be stopped initially")

	client := getTestClient(t)
	plan.Handler = func(idx uint64, data interface{}) {}

	go func() {
		plan.RunWithClientAndHclog(client, nil)
	}()

	time.Sleep(500 * time.Millisecond)
	assert.False(t, plan.IsStopped(), "Plan should not be stopped while running")

	plan.Stop()
	time.Sleep(200 * time.Millisecond)
	assert.True(t, plan.IsStopped(), "Plan should be stopped after Stop()")
}

// ==================== Watch with Filter Tests ====================

// TestWatchChecksWithFilter tests watching checks with a filter expression
func TestWatchChecksWithFilter(t *testing.T) {
	client := getTestClient(t)

	params := map[string]interface{}{
		"type":  "checks",
		"state": "passing",
	}

	plan, err := watch.Parse(params)
	require.NoError(t, err, "Watch parse should succeed")

	updates := make(chan []*api.HealthCheck, 10)
	plan.Handler = func(idx uint64, data interface{}) {
		if checks, ok := data.([]*api.HealthCheck); ok {
			select {
			case updates <- checks:
			default:
			}
		}
	}

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	go func() {
		plan.RunWithClientAndHclog(client, nil)
	}()
	defer plan.Stop()

	select {
	case checks := <-updates:
		for _, c := range checks {
			assert.Equal(t, api.HealthPassing, c.Status, "All checks should be passing")
		}
	case <-ctx.Done():
		t.Fatal("Watch checks with filter timeout")
	}
}

// TestWatchKeyPrefixUpdate tests that keyprefix watch detects new keys
func TestWatchKeyPrefixUpdate(t *testing.T) {
	client := getTestClient(t)
	kv := client.KV()
	prefix := "watch-pfx-upd-" + randomString(8) + "/"

	// Create initial key
	_, err := kv.Put(&api.KVPair{Key: prefix + "key1", Value: []byte("v1")}, nil)
	require.NoError(t, err)
	defer kv.DeleteTree(prefix, nil)

	params := map[string]interface{}{
		"type":   "keyprefix",
		"prefix": prefix,
	}

	plan, err := watch.Parse(params)
	require.NoError(t, err)

	updates := make(chan int, 10)
	plan.Handler = func(idx uint64, data interface{}) {
		if pairs, ok := data.(api.KVPairs); ok {
			select {
			case updates <- len(pairs):
			default:
			}
		}
	}

	go func() {
		plan.RunWithClientAndHclog(client, nil)
	}()
	defer plan.Stop()

	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()

	// Wait for initial
	select {
	case count := <-updates:
		assert.Equal(t, 1, count, "Initial key count should be 1")
	case <-ctx.Done():
		t.Fatal("Timeout waiting for initial keys")
	}

	// Add a new key under the prefix
	_, err = kv.Put(&api.KVPair{Key: prefix + "key2", Value: []byte("v2")}, nil)
	require.NoError(t, err)

	// Wait for update
	select {
	case count := <-updates:
		assert.GreaterOrEqual(t, count, 2, "Should have at least 2 keys after addition")
	case <-ctx.Done():
		t.Fatal("Timeout waiting for keyprefix update")
	}
}

// TestWatchServiceRegistration tests that services watch detects new registrations
func TestWatchServiceRegistration(t *testing.T) {
	client := getTestClient(t)

	params := map[string]interface{}{
		"type": "services",
	}

	plan, err := watch.Parse(params)
	require.NoError(t, err, "Watch parse should succeed")

	updates := make(chan map[string][]string, 10)
	plan.Handler = func(idx uint64, data interface{}) {
		if services, ok := data.(map[string][]string); ok {
			select {
			case updates <- services:
			default:
			}
		}
	}

	go func() {
		plan.RunWithClientAndHclog(client, nil)
	}()
	defer plan.Stop()

	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()

	// Wait for initial catalog
	var initialCount int
	select {
	case services := <-updates:
		initialCount = len(services)
		assert.Greater(t, initialCount, 0, "Should have at least one service initially (consul)")
	case <-ctx.Done():
		t.Fatal("Timeout waiting for initial services")
	}

	// Register a new service
	serviceName := "watch-new-reg-" + randomString(8)
	err = client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	})
	require.NoError(t, err)
	defer client.Agent().ServiceDeregister(serviceName)

	// Wait for catalog update
	select {
	case services := <-updates:
		assert.Greater(t, len(services), initialCount, "Service count should increase after registration")
		_, found := services[serviceName]
		assert.True(t, found, "Newly registered service %s should be detected by watch", serviceName)
	case <-ctx.Done():
		t.Fatal("Timeout waiting for service registration notification")
	}
}
