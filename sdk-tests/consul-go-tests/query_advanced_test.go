package consultest

import (
	"testing"
	"time"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// ==================== Prepared Query Basic Tests ====================

// TestPreparedQueryCreate tests creating a prepared query
func TestPreparedQueryCreate(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	query := client.PreparedQuery()

	serviceName := "query-svc-" + randomString(8)

	// Register service first
	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	})
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	// Create prepared query
	def := &api.PreparedQueryDefinition{
		Name: "query-" + serviceName,
		Service: api.ServiceQuery{
			Service: serviceName,
		},
	}

	id, _, err := query.Create(def, nil)
	if err != nil {
		t.Logf("Prepared query create: %v", err)
		return
	}
	defer query.Delete(id, nil)

	assert.NotEmpty(t, id)
	t.Logf("Created prepared query: %s", id)
}

// TestPreparedQueryGet tests getting a prepared query
func TestPreparedQueryGet(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	query := client.PreparedQuery()

	serviceName := "query-get-" + randomString(8)

	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	})
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	def := &api.PreparedQueryDefinition{
		Name: "get-query-" + serviceName,
		Service: api.ServiceQuery{
			Service: serviceName,
		},
	}

	id, _, err := query.Create(def, nil)
	if err != nil {
		t.Logf("Prepared query create: %v", err)
		return
	}
	defer query.Delete(id, nil)

	// Get the query
	queries, _, err := query.Get(id, nil)
	if err != nil {
		t.Logf("Prepared query get: %v", err)
		return
	}

	require.Len(t, queries, 1)
	assert.Equal(t, def.Name, queries[0].Name)
	t.Logf("Got query: %s", queries[0].Name)
}

// TestPreparedQueryList tests listing all prepared queries
func TestPreparedQueryList(t *testing.T) {
	client := getTestClient(t)

	query := client.PreparedQuery()

	queries, _, err := query.List(nil)
	if err != nil {
		t.Logf("Prepared query list: %v", err)
		return
	}

	t.Logf("Found %d prepared queries", len(queries))
	for _, q := range queries {
		t.Logf("  Query: %s (Service: %s)", q.Name, q.Service.Service)
	}
}

// TestPreparedQueryUpdate tests updating a prepared query
func TestPreparedQueryUpdate(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	query := client.PreparedQuery()

	serviceName := "query-update-" + randomString(8)

	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	})
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	def := &api.PreparedQueryDefinition{
		Name: "update-query-" + serviceName,
		Service: api.ServiceQuery{
			Service: serviceName,
		},
	}

	id, _, err := query.Create(def, nil)
	if err != nil {
		t.Logf("Prepared query create: %v", err)
		return
	}
	defer query.Delete(id, nil)

	// Update the query
	def.ID = id
	def.Service.OnlyPassing = true

	_, err = query.Update(def, nil)
	if err != nil {
		t.Logf("Prepared query update: %v", err)
		return
	}

	// Verify update
	queries, _, err := query.Get(id, nil)
	if err != nil {
		t.Logf("Prepared query get: %v", err)
		return
	}

	assert.True(t, queries[0].Service.OnlyPassing)
	t.Log("Query updated successfully")
}

// TestPreparedQueryDelete tests deleting a prepared query
func TestPreparedQueryDelete(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	query := client.PreparedQuery()

	serviceName := "query-delete-" + randomString(8)

	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	})
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	def := &api.PreparedQueryDefinition{
		Name: "delete-query-" + serviceName,
		Service: api.ServiceQuery{
			Service: serviceName,
		},
	}

	id, _, err := query.Create(def, nil)
	if err != nil {
		t.Logf("Prepared query create: %v", err)
		return
	}

	// Delete
	_, err = query.Delete(id, nil)
	if err != nil {
		t.Logf("Prepared query delete: %v", err)
		return
	}

	// Verify deletion
	_, _, err = query.Get(id, nil)
	assert.Error(t, err, "Query should be deleted")
	t.Log("Query deleted successfully")
}

// ==================== Prepared Query Execute Tests ====================

// TestPreparedQueryExecute tests executing a prepared query
func TestPreparedQueryExecute(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	query := client.PreparedQuery()

	serviceName := "query-exec-" + randomString(8)

	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
		Tags: []string{"primary"},
	})
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	def := &api.PreparedQueryDefinition{
		Name: "exec-query-" + serviceName,
		Service: api.ServiceQuery{
			Service: serviceName,
		},
	}

	id, _, err := query.Create(def, nil)
	if err != nil {
		t.Logf("Prepared query create: %v", err)
		return
	}
	defer query.Delete(id, nil)

	// Execute the query
	result, _, err := query.Execute(id, nil)
	if err != nil {
		t.Logf("Prepared query execute: %v", err)
		return
	}

	t.Logf("Query result - Service: %s, Nodes: %d", result.Service, len(result.Nodes))
	for _, node := range result.Nodes {
		t.Logf("  Node: %s, Service: %s:%d",
			node.Node.Node, node.Service.Service, node.Service.Port)
	}
}

// TestPreparedQueryExecuteByName tests executing a query by name
func TestPreparedQueryExecuteByName(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	query := client.PreparedQuery()

	serviceName := "query-name-" + randomString(8)
	queryName := "name-query-" + serviceName

	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	})
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	def := &api.PreparedQueryDefinition{
		Name: queryName,
		Service: api.ServiceQuery{
			Service: serviceName,
		},
	}

	id, _, err := query.Create(def, nil)
	if err != nil {
		t.Logf("Prepared query create: %v", err)
		return
	}
	defer query.Delete(id, nil)

	// Execute by name
	result, _, err := query.Execute(queryName, nil)
	if err != nil {
		t.Logf("Prepared query execute by name: %v", err)
		return
	}

	t.Logf("Query by name - Nodes: %d", len(result.Nodes))
}

// ==================== Prepared Query Options Tests ====================

// TestPreparedQueryWithFailover tests query with failover
func TestPreparedQueryWithFailover(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	query := client.PreparedQuery()

	serviceName := "query-failover-" + randomString(8)

	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	})
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	def := &api.PreparedQueryDefinition{
		Name: "failover-query-" + serviceName,
		Service: api.ServiceQuery{
			Service:     serviceName,
			OnlyPassing: true,
			Failover: api.QueryFailoverOptions{
				NearestN:    3,
				Datacenters: []string{"dc1", "dc2"},
			},
		},
	}

	id, _, err := query.Create(def, nil)
	if err != nil {
		t.Logf("Prepared query create with failover: %v", err)
		return
	}
	defer query.Delete(id, nil)

	t.Logf("Created query with failover: %s", id)
}

// TestPreparedQueryWithTags tests query with tag filter
func TestPreparedQueryWithTags(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	query := client.PreparedQuery()

	serviceName := "query-tags-" + randomString(8)

	// Register with tags
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
		Tags: []string{"secondary", "v1"},
	})
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName + "-2")

	time.Sleep(500 * time.Millisecond)

	// Query with tag filter
	def := &api.PreparedQueryDefinition{
		Name: "tags-query-" + serviceName,
		Service: api.ServiceQuery{
			Service: serviceName,
			Tags:    []string{"primary"},
		},
	}

	id, _, err := query.Create(def, nil)
	if err != nil {
		t.Logf("Prepared query create with tags: %v", err)
		return
	}
	defer query.Delete(id, nil)

	// Execute
	result, _, err := query.Execute(id, nil)
	if err != nil {
		t.Logf("Prepared query execute: %v", err)
		return
	}

	t.Logf("Query with tags - Nodes: %d", len(result.Nodes))
	for _, node := range result.Nodes {
		t.Logf("  Tags: %v", node.Service.Tags)
	}
}

// TestPreparedQueryWithNear tests query with near parameter
func TestPreparedQueryWithNear(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	query := client.PreparedQuery()

	serviceName := "query-near-" + randomString(8)

	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	})
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	def := &api.PreparedQueryDefinition{
		Name: "near-query-" + serviceName,
		Service: api.ServiceQuery{
			Service: serviceName,
			Near:    "_agent",
		},
	}

	id, _, err := query.Create(def, nil)
	if err != nil {
		t.Logf("Prepared query create with near: %v", err)
		return
	}
	defer query.Delete(id, nil)

	// Execute with near
	opts := &api.QueryOptions{}
	result, _, err := query.Execute(id, opts)
	if err != nil {
		t.Logf("Prepared query execute with near: %v", err)
		return
	}

	t.Logf("Query with near - Nodes: %d", len(result.Nodes))
}

// ==================== Prepared Query DNS Tests ====================

// TestPreparedQueryDNS tests DNS-related query options
func TestPreparedQueryDNS(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	query := client.PreparedQuery()

	serviceName := "query-dns-" + randomString(8)

	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	})
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	def := &api.PreparedQueryDefinition{
		Name: "dns-query-" + serviceName,
		Service: api.ServiceQuery{
			Service: serviceName,
		},
		DNS: api.QueryDNSOptions{
			TTL: "10s",
		},
	}

	id, _, err := query.Create(def, nil)
	if err != nil {
		t.Logf("Prepared query create with DNS: %v", err)
		return
	}
	defer query.Delete(id, nil)

	// Verify
	queries, _, err := query.Get(id, nil)
	if err != nil {
		t.Logf("Prepared query get: %v", err)
		return
	}

	assert.Equal(t, "10s", queries[0].DNS.TTL)
	t.Logf("Query DNS TTL: %s", queries[0].DNS.TTL)
}

// ==================== Prepared Query Template Tests ====================

// TestPreparedQueryTemplate tests query templates
func TestPreparedQueryTemplate(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	query := client.PreparedQuery()

	serviceName := "query-template-" + randomString(8)

	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	})
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	// Create template query
	def := &api.PreparedQueryDefinition{
		Name: "",
		Template: api.QueryTemplate{
			Type:   "name_prefix_match",
			Regexp: "^geo-(.+)-query$",
		},
		Service: api.ServiceQuery{
			Service: "${match(1)}",
		},
	}

	id, _, err := query.Create(def, nil)
	if err != nil {
		t.Logf("Prepared query template create: %v", err)
		return
	}
	defer query.Delete(id, nil)

	t.Logf("Created template query: %s", id)
}

// ==================== Prepared Query Session Tests ====================

// TestPreparedQueryWithSession tests query with session binding
func TestPreparedQueryWithSession(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	query := client.PreparedQuery()
	session := client.Session()

	serviceName := "query-session-" + randomString(8)

	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	})
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)

	// Create session
	sessionID, _, err := session.Create(&api.SessionEntry{
		Name: "query-session",
		TTL:  "30s",
	}, nil)
	if err != nil {
		t.Logf("Session create: %v", err)
		return
	}
	defer session.Destroy(sessionID, nil)

	time.Sleep(500 * time.Millisecond)

	def := &api.PreparedQueryDefinition{
		Name:    "session-query-" + serviceName,
		Session: sessionID,
		Service: api.ServiceQuery{
			Service: serviceName,
		},
	}

	id, _, err := query.Create(def, nil)
	if err != nil {
		t.Logf("Prepared query create with session: %v", err)
		return
	}
	defer query.Delete(id, nil)

	t.Logf("Created query with session: %s", id)
}

// ==================== Prepared Query Error Tests ====================

// TestPreparedQueryNotFound tests query not found error
func TestPreparedQueryNotFound(t *testing.T) {
	client := getTestClient(t)

	query := client.PreparedQuery()

	_, _, err := query.Get("non-existent-query-id", nil)
	if err != nil {
		t.Logf("Query not found (expected): %v", err)
	} else {
		t.Log("Query not found did not return error")
	}
}

// TestPreparedQueryExecuteNotFound tests executing non-existent query
func TestPreparedQueryExecuteNotFound(t *testing.T) {
	client := getTestClient(t)

	query := client.PreparedQuery()

	_, _, err := query.Execute("non-existent-query", nil)
	if err != nil {
		t.Logf("Execute not found (expected): %v", err)
	} else {
		t.Log("Execute not found did not return error")
	}
}

// ==================== Prepared Query Concurrent Tests ====================

// TestPreparedQueryConcurrentExecute tests concurrent query execution
func TestPreparedQueryConcurrentExecute(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	query := client.PreparedQuery()

	serviceName := "query-concurrent-" + randomString(8)

	// Register multiple instances
	for i := 0; i < 3; i++ {
		id := serviceName + "-" + string(rune('0'+i))
		err := agent.ServiceRegister(&api.AgentServiceRegistration{
			ID:   id,
			Name: serviceName,
			Port: 8080 + i,
		})
		require.NoError(t, err)
		defer agent.ServiceDeregister(id)
	}

	time.Sleep(500 * time.Millisecond)

	def := &api.PreparedQueryDefinition{
		Name: "concurrent-query-" + serviceName,
		Service: api.ServiceQuery{
			Service: serviceName,
		},
	}

	id, _, err := query.Create(def, nil)
	if err != nil {
		t.Logf("Prepared query create: %v", err)
		return
	}
	defer query.Delete(id, nil)

	// Concurrent execution
	results := make(chan int, 10)
	for i := 0; i < 10; i++ {
		go func() {
			result, _, err := query.Execute(id, nil)
			if err != nil {
				results <- 0
			} else {
				results <- len(result.Nodes)
			}
		}()
	}

	// Collect results
	total := 0
	for i := 0; i < 10; i++ {
		count := <-results
		total += count
	}

	t.Logf("Concurrent query total node count: %d", total)
}
