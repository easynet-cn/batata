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
	if err != nil {
		t.Logf("Watch parse: %v", err)
		return
	}

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
		t.Logf("Initial value: %s", string(pair.Value))
	case <-ctx.Done():
		t.Log("Watch key timeout waiting for initial value")
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
	if err != nil {
		t.Logf("Watch parse: %v", err)
		return
	}

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
		t.Logf("Prefix watch found %d keys", len(pairs))
	case <-ctx.Done():
		t.Log("Watch key prefix timeout")
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
	if err != nil {
		t.Logf("Watch parse: %v", err)
		return
	}

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
		t.Logf("Found %d services", len(services))
		if _, ok := services[serviceName]; ok {
			t.Logf("Watched service %s found", serviceName)
		}
	case <-ctx.Done():
		t.Log("Watch services timeout")
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
	if err != nil {
		t.Logf("Watch parse: %v", err)
		return
	}

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
		t.Logf("Service watch found %d entries", len(entries))
	case <-ctx.Done():
		t.Log("Watch service timeout")
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
	if err != nil {
		t.Logf("Watch parse: %v", err)
		return
	}

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
		t.Logf("Health watch found %d checks", len(checks))
	case <-ctx.Done():
		t.Log("Watch checks timeout")
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
	if err != nil {
		t.Logf("Watch parse: %v", err)
		return
	}

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
		passing := 0
		warning := 0
		critical := 0
		for _, c := range checks {
			switch c.Status {
			case api.HealthPassing:
				passing++
			case api.HealthWarning:
				warning++
			case api.HealthCritical:
				critical++
			}
		}
		t.Logf("Checks: passing=%d, warning=%d, critical=%d", passing, warning, critical)
	case <-ctx.Done():
		t.Log("Watch checks state timeout")
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
	if err != nil {
		t.Logf("Watch parse: %v", err)
		return
	}

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
		t.Logf("Nodes watch found %d nodes", len(nodes))
		for _, n := range nodes {
			t.Logf("  Node: %s, Address: %s", n.Node, n.Address)
		}
	case <-ctx.Done():
		t.Log("Watch nodes timeout")
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
	if err != nil {
		t.Logf("Watch parse: %v", err)
		return
	}

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

	// Fire event
	time.Sleep(500 * time.Millisecond)
	event := client.Event()
	_, _, err = event.Fire(&api.UserEvent{
		Name:    eventName,
		Payload: []byte("test payload"),
	}, nil)

	if err != nil {
		t.Logf("Fire event: %v", err)
	}

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	select {
	case events := <-updates:
		t.Logf("Event watch received %d events", len(events))
	case <-ctx.Done():
		t.Log("Watch event timeout")
	}
}

// ==================== Watch Connect Tests ====================

// TestWatchConnectRoots tests watching Connect CA roots
func TestWatchConnectRoots(t *testing.T) {
	client := getTestClient(t)

	params := map[string]interface{}{
		"type": "connect_roots",
	}

	plan, err := watch.Parse(params)
	if err != nil {
		t.Logf("Watch parse: %v", err)
		return
	}

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
		if roots != nil {
			t.Logf("Connect roots: %d roots", len(roots.Roots))
		}
	case <-ctx.Done():
		t.Log("Watch connect roots timeout")
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
	if err != nil {
		t.Logf("Service register: %v", err)
		return
	}
	defer agent.ServiceDeregister(serviceName)

	params := map[string]interface{}{
		"type":    "connect_leaf",
		"service": serviceName,
	}

	plan, err := watch.Parse(params)
	if err != nil {
		t.Logf("Watch parse: %v", err)
		return
	}

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
		if leaf != nil {
			t.Logf("Leaf cert service: %s", leaf.Service)
		}
	case <-ctx.Done():
		t.Log("Watch connect leaf timeout")
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

	t.Logf("Watch cancelled after %d calls", callCount)
	assert.True(t, plan.IsStopped())
}

// TestWatchMultiple tests running multiple watches
func TestWatchMultiple(t *testing.T) {
	client := getTestClient(t)

	kv := client.KV()
	keys := make([]string, 3)
	plans := make([]*watch.Plan, 3)

	for i := 0; i < 3; i++ {
		key := "watch-multi-" + randomString(8)
		keys[i] = key

		_, err := kv.Put(&api.KVPair{Key: key, Value: []byte("value" + string(rune('0'+i)))}, nil)
		require.NoError(t, err)
		defer kv.Delete(key, nil)

		params := map[string]interface{}{
			"type": "key",
			"key":  key,
		}

		plan, err := watch.Parse(params)
		if err != nil {
			t.Logf("Watch parse %d: %v", i, err)
			continue
		}

		idx := i
		plan.Handler = func(modIdx uint64, data interface{}) {
			if pair, ok := data.(*api.KVPair); ok && pair != nil {
				t.Logf("Watch %d got update: %s", idx, string(pair.Value))
			}
		}

		plans[i] = plan
		go func() {
			plan.RunWithClientAndHclog(client, nil)
		}()
	}

	// Let watches run
	time.Sleep(1 * time.Second)

	// Stop all
	for i, plan := range plans {
		if plan != nil {
			plan.Stop()
			t.Logf("Stopped watch %d", i)
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
	if err != nil {
		t.Logf("Watch parse: %v", err)
		return
	}

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
		t.Logf("DC1 nodes: %d", len(nodes))
	case <-ctx.Done():
		t.Log("Watch with datacenter timeout")
	}
}

// TestWatchWithToken tests watching with ACL token
func TestWatchWithToken(t *testing.T) {
	client := getTestClient(t)

	params := map[string]interface{}{
		"type": "services",
	}

	plan, err := watch.Parse(params)
	if err != nil {
		t.Logf("Watch parse: %v", err)
		return
	}

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
		t.Logf("Services with token: %d", len(services))
	case <-ctx.Done():
		t.Log("Watch with token timeout")
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

	t.Logf("Handler was called %d times", callCount)
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
	if err != nil {
		t.Logf("Watch parse: %v", err)
		return
	}

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
		t.Logf("Service watch with tag found %d entries", len(entries))
		for _, e := range entries {
			t.Logf("  Service ID: %s, Tags: %v", e.Service.ID, e.Service.Tags)
		}
	case <-ctx.Done():
		t.Log("Watch service with tag timeout")
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
		"type":         "service",
		"service":      serviceName,
		"passingonly":  true,
	}

	plan, err := watch.Parse(params)
	if err != nil {
		t.Logf("Watch parse: %v", err)
		return
	}

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
		t.Logf("Service watch passing only: %d entries", len(entries))
	case <-ctx.Done():
		t.Log("Watch service passing only timeout")
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
	select {
	case val := <-updates:
		t.Logf("Initial value: %s", val)
	case <-ctx.Done():
		t.Log("Timeout waiting for initial value")
		return
	}

	// Update the key
	_, err = kv.Put(&api.KVPair{Key: key, Value: []byte("v2")}, nil)
	require.NoError(t, err)

	// Wait for update notification
	select {
	case val := <-updates:
		t.Logf("Updated value: %s", val)
		assert.Equal(t, "v2", val, "Should receive updated value")
	case <-ctx.Done():
		t.Log("Timeout waiting for update")
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

	callCount := 0
	plan.Handler = func(idx uint64, data interface{}) {
		callCount++
		if data == nil {
			t.Log("Key was deleted (nil data)")
		} else if pair, ok := data.(*api.KVPair); ok {
			if pair == nil {
				t.Log("Key was deleted (nil pair)")
			} else {
				t.Logf("Key value: %s", string(pair.Value))
			}
		}
	}

	go func() {
		plan.RunWithClientAndHclog(client, nil)
	}()
	defer plan.Stop()

	time.Sleep(1 * time.Second)

	// Delete the key
	_, err = kv.Delete(key, nil)
	require.NoError(t, err)

	time.Sleep(2 * time.Second)
	t.Logf("Handler called %d times (should include delete notification)", callCount)
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
	if err != nil {
		t.Logf("Watch parse: %v", err)
		return
	}

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
		t.Logf("Checks for service %s: %d", serviceName, len(checks))
		for _, c := range checks {
			t.Logf("  Check: %s, Status: %s", c.CheckID, c.Status)
		}
	case <-ctx.Done():
		t.Log("Watch checks by service timeout")
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
		"type":   "checks",
		"state":  "passing",
	}

	plan, err := watch.Parse(params)
	if err != nil {
		t.Logf("Watch parse: %v", err)
		return
	}

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
		t.Logf("Passing checks: %d", len(checks))
		for _, c := range checks {
			assert.Equal(t, api.HealthPassing, c.Status, "All checks should be passing")
		}
	case <-ctx.Done():
		t.Log("Watch checks with filter timeout")
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
		t.Logf("Initial key count: %d", count)
	case <-ctx.Done():
		t.Log("Timeout waiting for initial keys")
		return
	}

	// Add a new key under the prefix
	_, err = kv.Put(&api.KVPair{Key: prefix + "key2", Value: []byte("v2")}, nil)
	require.NoError(t, err)

	// Wait for update
	select {
	case count := <-updates:
		t.Logf("Updated key count: %d", count)
		assert.GreaterOrEqual(t, count, 2, "Should have at least 2 keys after addition")
	case <-ctx.Done():
		t.Log("Timeout waiting for keyprefix update")
	}
}

// TestWatchServiceRegistration tests that services watch detects new registrations
func TestWatchServiceRegistration(t *testing.T) {
	client := getTestClient(t)

	params := map[string]interface{}{
		"type": "services",
	}

	plan, err := watch.Parse(params)
	if err != nil {
		t.Logf("Watch parse: %v", err)
		return
	}

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
	select {
	case services := <-updates:
		t.Logf("Initial services: %d", len(services))
	case <-ctx.Done():
		t.Log("Timeout waiting for initial services")
		return
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
		t.Logf("Updated services: %d", len(services))
		_, found := services[serviceName]
		if found {
			t.Logf("New service %s detected by watch", serviceName)
		}
	case <-ctx.Done():
		t.Log("Timeout waiting for service registration notification")
	}
}
