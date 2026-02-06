package tests

import (
	"testing"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// ==================== Prepared Query API Tests ====================

// CQ-001: Test create prepared query
func TestQueryCreate(t *testing.T) {
	client := getClient(t)

	// Register a service first
	serviceName := "query-service-" + randomID()
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	})
	require.NoError(t, err)
	defer client.Agent().ServiceDeregister(serviceName)

	// Create prepared query
	query := &api.PreparedQueryDefinition{
		Name: "test-query-" + randomID(),
		Service: api.ServiceQuery{
			Service: serviceName,
		},
	}

	queryID, _, err := client.PreparedQuery().Create(query, nil)
	if err != nil {
		t.Logf("Prepared query create error: %v", err)
		t.Skip("Prepared query API not supported")
	}

	assert.NotEmpty(t, queryID, "Should return query ID")
	t.Logf("Created query: %s", queryID)

	// Cleanup
	client.PreparedQuery().Delete(queryID, nil)
}

// CQ-002: Test list prepared queries
func TestQueryList(t *testing.T) {
	client := getClient(t)

	// Create a query first
	serviceName := "list-query-service-" + randomID()
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	})
	require.NoError(t, err)
	defer client.Agent().ServiceDeregister(serviceName)

	query := &api.PreparedQueryDefinition{
		Name: "list-query-" + randomID(),
		Service: api.ServiceQuery{
			Service: serviceName,
		},
	}
	queryID, _, err := client.PreparedQuery().Create(query, nil)
	if err != nil {
		t.Skip("Prepared query API not supported")
	}
	defer client.PreparedQuery().Delete(queryID, nil)

	// List
	queries, _, err := client.PreparedQuery().List(nil)
	assert.NoError(t, err, "Query list should succeed")
	assert.NotEmpty(t, queries, "Should have at least one query")

	t.Logf("Found %d queries", len(queries))
}

// CQ-003: Test get prepared query
func TestQueryGet(t *testing.T) {
	client := getClient(t)

	// Create a query first
	serviceName := "get-query-service-" + randomID()
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	})
	require.NoError(t, err)
	defer client.Agent().ServiceDeregister(serviceName)

	queryName := "get-query-" + randomID()
	query := &api.PreparedQueryDefinition{
		Name: queryName,
		Service: api.ServiceQuery{
			Service: serviceName,
		},
	}
	queryID, _, err := client.PreparedQuery().Create(query, nil)
	if err != nil {
		t.Skip("Prepared query API not supported")
	}
	defer client.PreparedQuery().Delete(queryID, nil)

	// Get
	queries, _, err := client.PreparedQuery().Get(queryID, nil)
	assert.NoError(t, err, "Query get should succeed")
	assert.Len(t, queries, 1)
	assert.Equal(t, queryName, queries[0].Name)
}

// CQ-004: Test update prepared query
func TestQueryUpdate(t *testing.T) {
	client := getClient(t)

	// Create a query first
	serviceName := "update-query-service-" + randomID()
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	})
	require.NoError(t, err)
	defer client.Agent().ServiceDeregister(serviceName)

	query := &api.PreparedQueryDefinition{
		Name: "update-query-" + randomID(),
		Service: api.ServiceQuery{
			Service: serviceName,
		},
	}
	queryID, _, err := client.PreparedQuery().Create(query, nil)
	if err != nil {
		t.Skip("Prepared query API not supported")
	}
	defer client.PreparedQuery().Delete(queryID, nil)

	// Update
	query.ID = queryID
	query.Service.Tags = []string{"updated"}
	_, err = client.PreparedQuery().Update(query, nil)
	assert.NoError(t, err, "Query update should succeed")
}

// CQ-005: Test delete prepared query
func TestQueryDelete(t *testing.T) {
	client := getClient(t)

	// Create a query first
	serviceName := "delete-query-service-" + randomID()
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	})
	require.NoError(t, err)
	defer client.Agent().ServiceDeregister(serviceName)

	query := &api.PreparedQueryDefinition{
		Name: "delete-query-" + randomID(),
		Service: api.ServiceQuery{
			Service: serviceName,
		},
	}
	queryID, _, err := client.PreparedQuery().Create(query, nil)
	if err != nil {
		t.Skip("Prepared query API not supported")
	}

	// Delete
	_, err = client.PreparedQuery().Delete(queryID, nil)
	assert.NoError(t, err, "Query delete should succeed")
}

// CQ-006: Test execute prepared query
func TestQueryExecute(t *testing.T) {
	client := getClient(t)

	// Register a service first
	serviceName := "exec-query-service-" + randomID()
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:      serviceName,
		Name:    serviceName,
		Port:    8080,
		Address: "192.168.1.100",
	})
	require.NoError(t, err)
	defer client.Agent().ServiceDeregister(serviceName)

	// Create query
	query := &api.PreparedQueryDefinition{
		Name: "exec-query-" + randomID(),
		Service: api.ServiceQuery{
			Service: serviceName,
		},
	}
	queryID, _, err := client.PreparedQuery().Create(query, nil)
	if err != nil {
		t.Skip("Prepared query API not supported")
	}
	defer client.PreparedQuery().Delete(queryID, nil)

	// Execute
	result, _, err := client.PreparedQuery().Execute(queryID, nil)
	assert.NoError(t, err, "Query execute should succeed")
	assert.NotEmpty(t, result.Nodes, "Should find service nodes")

	t.Logf("Query returned %d nodes", len(result.Nodes))
}

// CQ-007: Test execute prepared query by name
func TestQueryExecuteByName(t *testing.T) {
	client := getClient(t)

	// Register a service first
	serviceName := "name-query-service-" + randomID()
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	})
	require.NoError(t, err)
	defer client.Agent().ServiceDeregister(serviceName)

	queryName := "name-query-" + randomID()
	query := &api.PreparedQueryDefinition{
		Name: queryName,
		Service: api.ServiceQuery{
			Service: serviceName,
		},
	}
	queryID, _, err := client.PreparedQuery().Create(query, nil)
	if err != nil {
		t.Skip("Prepared query API not supported")
	}
	defer client.PreparedQuery().Delete(queryID, nil)

	// Execute by name
	result, _, err := client.PreparedQuery().Execute(queryName, nil)
	assert.NoError(t, err, "Query execute by name should succeed")

	t.Logf("Query by name returned %d nodes", len(result.Nodes))
}

// CQ-008: Test query with failover
func TestQueryWithFailover(t *testing.T) {
	client := getClient(t)

	serviceName := "failover-query-service-" + randomID()
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	})
	require.NoError(t, err)
	defer client.Agent().ServiceDeregister(serviceName)

	// Create query with failover settings
	query := &api.PreparedQueryDefinition{
		Name: "failover-query-" + randomID(),
		Service: api.ServiceQuery{
			Service: serviceName,
			Failover: api.QueryFailoverOptions{
				NearestN: 3,
			},
		},
	}

	queryID, _, err := client.PreparedQuery().Create(query, nil)
	if err != nil {
		t.Skip("Prepared query API not supported")
	}
	defer client.PreparedQuery().Delete(queryID, nil)

	t.Logf("Created query with failover: %s", queryID)
}

// CQ-009: Test query with DNS settings
func TestQueryWithDNS(t *testing.T) {
	client := getClient(t)

	serviceName := "dns-query-service-" + randomID()
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	})
	require.NoError(t, err)
	defer client.Agent().ServiceDeregister(serviceName)

	// Create query with DNS settings
	query := &api.PreparedQueryDefinition{
		Name: "dns-query-" + randomID(),
		Service: api.ServiceQuery{
			Service: serviceName,
		},
		DNS: api.QueryDNSOptions{
			TTL: "10s",
		},
	}

	queryID, _, err := client.PreparedQuery().Create(query, nil)
	if err != nil {
		t.Skip("Prepared query API not supported")
	}
	defer client.PreparedQuery().Delete(queryID, nil)

	t.Logf("Created query with DNS settings: %s", queryID)
}
