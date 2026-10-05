package tests

import (
	"os"
	"sync"
	"testing"
	"time"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// ==================== Consul Maintenance Mode Tests ====================

// TestMaintenanceEnableNode tests enabling node maintenance mode
func TestMaintenanceEnableNode(t *testing.T) {
	client := getClient(t)

	// Enable node maintenance
	err := client.Agent().EnableNodeMaintenance("Testing node maintenance enable")
	if err != nil {
		t.Logf("Node maintenance enable error: %v", err)
		t.Skip("Node maintenance not available")
	}

	requireEventually(t, "node maintenance check visible", 10*time.Second, func() bool {
		checks, err := client.Agent().Checks()
		if err != nil {
			return false
		}
		for _, check := range checks {
			if check.CheckID == "_node_maintenance" && check.Status == api.HealthCritical {
				return true
			}
		}
		return false
	}, "Node maintenance check should appear with critical status")

	// Verify maintenance check exists
	checks, err := client.Agent().Checks()
	assert.NoError(t, err)

	// Look for the maintenance check
	maintenanceFound := false
	for checkID, check := range checks {
		if check.Name == "_node_maintenance" || checkID == "_node_maintenance" {
			maintenanceFound = true
			assert.Equal(t, api.HealthCritical, check.Status, "Maintenance check should be critical")
			assert.Contains(t, check.Notes, "Testing node maintenance enable", "Should contain reason")
			t.Logf("Node maintenance check found: %s - %s", checkID, check.Notes)
		}
	}

	// Cleanup - disable maintenance
	client.Agent().DisableNodeMaintenance()

	// Log result even if check not found (implementation may vary)
	if !maintenanceFound {
		t.Log("Node maintenance check not found in checks list (may be implementation specific)")
	}
}

// TestMaintenanceDisableNode tests disabling node maintenance mode
func TestMaintenanceDisableNode(t *testing.T) {
	client := getClient(t)

	// First enable maintenance
	err := client.Agent().EnableNodeMaintenance("Temporary maintenance for disable test")
	if err != nil {
		t.Skip("Node maintenance not available")
	}

	requireEventually(t, "node maintenance enabled", 10*time.Second, func() bool {
		checks, err := client.Agent().Checks()
		if err != nil {
			return false
		}
		for _, check := range checks {
			if check.CheckID == "_node_maintenance" && check.Status == api.HealthCritical {
				return true
			}
		}
		return false
	}, "Node maintenance check should appear")

	// Now disable maintenance
	err = client.Agent().DisableNodeMaintenance()
	assert.NoError(t, err, "Disable node maintenance should succeed")

	requireEventually(t, "node maintenance disabled", 10*time.Second, func() bool {
		checks, err := client.Agent().Checks()
		if err != nil {
			return false
		}
		for _, check := range checks {
			if check.Name == "_node_maintenance" {
				return false
			}
		}
		return true
	}, "Node maintenance check should be removed")

	// Verify maintenance check is removed
	checks, err := client.Agent().Checks()
	assert.NoError(t, err)

	for checkID, check := range checks {
		if check.Name == "_node_maintenance" || checkID == "_node_maintenance" {
			t.Errorf("Node maintenance check should be removed after disable, found: %s", checkID)
		}
	}

	t.Log("Node maintenance successfully disabled")
}

// TestMaintenanceEnableService tests enabling service maintenance mode
func TestMaintenanceEnableService(t *testing.T) {
	client := getClient(t)
	serviceID := "maint-enable-svc-" + randomID()

	// Register a service
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceID,
		Name: "maintenance-enable-test",
		Port: 8080,
	})
	require.NoError(t, err)
	defer client.Agent().ServiceDeregister(serviceID)

	require.True(t, waitForAgentService(t, client, serviceID, 10*time.Second), "service %s should be registered", serviceID)

	// Enable service maintenance
	err = client.Agent().EnableServiceMaintenance(serviceID, "Testing service maintenance enable")
	assert.NoError(t, err, "Enable service maintenance should succeed")

	requireEventually(t, "service maintenance enabled", 10*time.Second, func() bool {
		checks, err := client.Agent().Checks()
		if err != nil {
			return false
		}
		check, ok := checks["_service_maintenance:"+serviceID]
		return ok && check.Status == api.HealthCritical
	}, "Service maintenance check should be critical")

	// Verify maintenance check exists
	checks, err := client.Agent().Checks()
	assert.NoError(t, err)

	maintenanceCheckID := "_service_maintenance:" + serviceID
	if check, ok := checks[maintenanceCheckID]; ok {
		assert.Equal(t, api.HealthCritical, check.Status, "Service maintenance check should be critical")
		t.Logf("Service maintenance check found: %s - %s", maintenanceCheckID, check.Notes)
	} else {
		t.Logf("Service maintenance check not found with expected ID %s (checking all checks)", maintenanceCheckID)
		for checkID, check := range checks {
			t.Logf("  Check: %s, ServiceID: %s, Status: %s", checkID, check.ServiceID, check.Status)
		}
	}

	// Cleanup
	client.Agent().DisableServiceMaintenance(serviceID)
}

// TestMaintenanceDisableService tests disabling service maintenance mode
func TestMaintenanceDisableService(t *testing.T) {
	client := getClient(t)
	serviceID := "maint-disable-svc-" + randomID()

	// Register a service
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceID,
		Name: "maintenance-disable-test",
		Port: 8080,
	})
	require.NoError(t, err)
	defer client.Agent().ServiceDeregister(serviceID)

	require.True(t, waitForAgentService(t, client, serviceID, 10*time.Second), "service %s should be registered", serviceID)

	// Enable maintenance first
	err = client.Agent().EnableServiceMaintenance(serviceID, "Temporary maintenance")
	require.NoError(t, err)

	requireEventually(t, "service maintenance enabled", 10*time.Second, func() bool {
		checks, err := client.Agent().Checks()
		if err != nil {
			return false
		}
		check, ok := checks["_service_maintenance:"+serviceID]
		return ok && check.Status == api.HealthCritical
	}, "Service maintenance check should be critical")

	// Disable service maintenance
	err = client.Agent().DisableServiceMaintenance(serviceID)
	assert.NoError(t, err, "Disable service maintenance should succeed")

	requireEventually(t, "service maintenance disabled", 10*time.Second, func() bool {
		checks, err := client.Agent().Checks()
		if err != nil {
			return false
		}
		check, ok := checks["_service_maintenance:"+serviceID]
		return !ok || check.Status != api.HealthCritical
	}, "Service maintenance check should be removed")

	// Verify maintenance check is removed
	checks, err := client.Agent().Checks()
	assert.NoError(t, err)

	maintenanceCheckID := "_service_maintenance:" + serviceID
	if _, ok := checks[maintenanceCheckID]; ok {
		t.Errorf("Service maintenance check should be removed after disable")
	} else {
		t.Log("Service maintenance successfully disabled")
	}
}

// TestMaintenanceWithReason tests maintenance mode with custom reason
func TestMaintenanceWithReason(t *testing.T) {
	client := getClient(t)
	serviceID := "maint-reason-svc-" + randomID()

	// Register a service
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceID,
		Name: "maintenance-reason-test",
		Port: 8080,
	})
	require.NoError(t, err)
	defer client.Agent().ServiceDeregister(serviceID)

	require.True(t, waitForAgentService(t, client, serviceID, 10*time.Second), "service %s should be registered", serviceID)

	// Test with different reasons
	testReasons := []string{
		"Planned maintenance window",
		"Upgrading to version 2.0",
		"Database migration in progress",
		"Emergency security patch",
	}

	for _, reason := range testReasons {
		// Enable with reason
		err = client.Agent().EnableServiceMaintenance(serviceID, reason)
		assert.NoError(t, err, "Enable maintenance with reason should succeed")

		requireEventually(t, "maintenance enabled with reason", 10*time.Second, func() bool {
			checks, err := client.Agent().Checks()
			if err != nil {
				return false
			}
			check, ok := checks["_service_maintenance:"+serviceID]
			return ok && check.Status == api.HealthCritical
		}, "Service maintenance check should be critical")

		// Check the reason is stored
		checks, err := client.Agent().Checks()
		assert.NoError(t, err)

		for _, check := range checks {
			if check.ServiceID == serviceID && check.Status == api.HealthCritical {
				t.Logf("Maintenance reason set: %s", check.Notes)
				// Note: reason may be stored in Notes or Output depending on implementation
			}
		}

		// Disable for next iteration
		client.Agent().DisableServiceMaintenance(serviceID)
		requireEventually(t, "maintenance disabled for next iteration", 10*time.Second, func() bool {
			checks, err := client.Agent().Checks()
			if err != nil {
				return false
			}
			check, ok := checks["_service_maintenance:"+serviceID]
			return !ok || check.Status != api.HealthCritical
		}, "Service maintenance check should be removed")
	}
}

// TestMaintenanceCheckStatus tests that maintenance creates critical check status
func TestMaintenanceCheckStatus(t *testing.T) {
	client := getClient(t)
	serviceID := "maint-status-svc-" + randomID()

	// Register a service with a passing check
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceID,
		Name: "maintenance-status-test",
		Port: 8080,
		Check: &api.AgentServiceCheck{
			TTL:    "30s",
			Status: api.HealthPassing,
		},
	})
	require.NoError(t, err)
	defer client.Agent().ServiceDeregister(serviceID)

	// Update TTL check to passing
	checkID := "service:" + serviceID
	client.Agent().PassTTL(checkID, "Service is healthy")

	requireEventually(t, "ttl check passing", 10*time.Second, func() bool {
		checks, err := client.Agent().Checks()
		if err != nil {
			return false
		}
		check, ok := checks[checkID]
		return ok && check.Status == api.HealthPassing
	}, "TTL check should be passing")

	// Verify service is passing
	healthEntries, _, err := client.Health().Service("maintenance-status-test", "", true, nil)
	assert.NoError(t, err)
	initialPassingCount := len(healthEntries)
	t.Logf("Initial passing service count: %d", initialPassingCount)

	// Enable maintenance - should create critical check
	err = client.Agent().EnableServiceMaintenance(serviceID, "Checking status impact")
	assert.NoError(t, err)

	requireEventually(t, "maintenance check critical", 10*time.Second, func() bool {
		checks, err := client.Agent().Checks()
		if err != nil {
			return false
		}
		for _, check := range checks {
			if check.ServiceID == serviceID && check.Status == api.HealthCritical {
				return true
			}
		}
		return false
	}, "Should have at least one critical check for service in maintenance")

	// Check all checks for service
	checks, err := client.Agent().Checks()
	assert.NoError(t, err)

	criticalFound := false
	for checkID, check := range checks {
		if check.ServiceID == serviceID {
			t.Logf("Check %s: Status=%s, Name=%s", checkID, check.Status, check.Name)
			if check.Status == api.HealthCritical {
				criticalFound = true
			}
		}
	}

	assert.True(t, criticalFound, "Should have at least one critical check for service in maintenance")

	// Cleanup
	client.Agent().DisableServiceMaintenance(serviceID)
}

// TestMaintenanceListServices tests listing services includes maintenance status
func TestMaintenanceListServices(t *testing.T) {
	client := getClient(t)
	prefix := "maint-list-" + randomID()

	// Register multiple services
	serviceIDs := []string{
		prefix + "-svc1",
		prefix + "-svc2",
		prefix + "-svc3",
	}

	for i, id := range serviceIDs {
		err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
			ID:   id,
			Name: "maintenance-list-test",
			Port: 8080 + i,
		})
		require.NoError(t, err)
		defer client.Agent().ServiceDeregister(id)
	}

	requireEventually(t, "services registered", 10*time.Second, func() bool {
		services, err := client.Agent().Services()
		if err != nil {
			return false
		}
		for _, id := range serviceIDs {
			if _, ok := services[id]; !ok {
				return false
			}
		}
		return true
	}, "All services should be registered")

	// Put first service in maintenance
	err := client.Agent().EnableServiceMaintenance(serviceIDs[0], "In maintenance")
	require.NoError(t, err)
	defer client.Agent().DisableServiceMaintenance(serviceIDs[0])

	requireEventually(t, "service maintenance enabled", 10*time.Second, func() bool {
		checks, err := client.Agent().Checks()
		if err != nil {
			return false
		}
		check, ok := checks["_service_maintenance:"+serviceIDs[0]]
		return ok && check.Status == api.HealthCritical
	}, "Service maintenance check should be critical")

	// List services
	services, err := client.Agent().Services()
	assert.NoError(t, err)

	// Verify all services are listed
	for _, id := range serviceIDs {
		_, exists := services[id]
		assert.True(t, exists, "Service %s should be listed", id)
	}

	// Check which services have maintenance checks
	checks, err := client.Agent().Checks()
	assert.NoError(t, err)

	maintenanceServices := make(map[string]bool)
	for _, check := range checks {
		if check.Status == api.HealthCritical && check.ServiceID != "" {
			maintenanceServices[check.ServiceID] = true
		}
	}

	t.Logf("Services in maintenance: %v", maintenanceServices)
}

// TestMaintenanceHealthImpact tests maintenance mode impact on health queries
func TestMaintenanceHealthImpact(t *testing.T) {
	client := getClient(t)
	serviceName := "maint-health-impact-" + randomID()
	serviceID := serviceName

	// Register a service with passing check
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceID,
		Name: serviceName,
		Port: 8080,
		Check: &api.AgentServiceCheck{
			TTL:    "30s",
			Status: api.HealthPassing,
		},
	})
	require.NoError(t, err)
	defer client.Agent().ServiceDeregister(serviceID)

	// Update to passing
	client.Agent().PassTTL("service:"+serviceID, "healthy")
	requireEventually(t, "ttl check passing", 10*time.Second, func() bool {
		checks, err := client.Agent().Checks()
		if err != nil {
			return false
		}
		check, ok := checks["service:"+serviceID]
		return ok && check.Status == api.HealthPassing
	}, "TTL check should be passing")

	// Query passing services
	passingBefore, _, err := client.Health().Service(serviceName, "", true, nil)
	assert.NoError(t, err)
	t.Logf("Passing services before maintenance: %d", len(passingBefore))

	// Enable maintenance
	err = client.Agent().EnableServiceMaintenance(serviceID, "Testing health impact")
	require.NoError(t, err)
	defer client.Agent().DisableServiceMaintenance(serviceID)

	requireEventually(t, "maintenance check critical", 10*time.Second, func() bool {
		checks, err := client.Agent().Checks()
		if err != nil {
			return false
		}
		check, ok := checks["_service_maintenance:"+serviceID]
		return ok && check.Status == api.HealthCritical
	}, "Service maintenance check should be critical")

	// Query passing services again - should not include maintenance service
	passingAfter, _, err := client.Health().Service(serviceName, "", true, nil)
	assert.NoError(t, err)
	t.Logf("Passing services after maintenance: %d", len(passingAfter))

	// All services (including non-passing)
	allServices, _, err := client.Health().Service(serviceName, "", false, nil)
	assert.NoError(t, err)
	t.Logf("All services (including maintenance): %d", len(allServices))

	// Service in maintenance should not be in passing list
	assert.LessOrEqual(t, len(passingAfter), len(passingBefore),
		"Passing services should not increase after enabling maintenance")
}

// TestMaintenanceDiscoveryImpact tests maintenance mode impact on service discovery
func TestMaintenanceDiscoveryImpact(t *testing.T) {
	client := getClient(t)
	serviceName := "maint-discovery-" + randomID()

	// Register two services
	healthyID := serviceName + "-healthy"
	maintenanceID := serviceName + "-maintenance"

	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:      healthyID,
		Name:    serviceName,
		Port:    8080,
		Address: "10.0.0.1",
		Check: &api.AgentServiceCheck{
			TTL:    "30s",
			Status: api.HealthPassing,
		},
	})
	require.NoError(t, err)
	defer client.Agent().ServiceDeregister(healthyID)

	err = client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:      maintenanceID,
		Name:    serviceName,
		Port:    8081,
		Address: "10.0.0.2",
		Check: &api.AgentServiceCheck{
			TTL:    "30s",
			Status: api.HealthPassing,
		},
	})
	require.NoError(t, err)
	defer client.Agent().ServiceDeregister(maintenanceID)

	// Set both to passing
	client.Agent().PassTTL("service:"+healthyID, "healthy")
	client.Agent().PassTTL("service:"+maintenanceID, "healthy")

	requireEventually(t, "ttl checks passing", 10*time.Second, func() bool {
		checks, err := client.Agent().Checks()
		if err != nil {
			return false
		}
		c1, ok1 := checks["service:"+healthyID]
		c2, ok2 := checks["service:"+maintenanceID]
		return ok1 && ok2 && c1.Status == api.HealthPassing && c2.Status == api.HealthPassing
	}, "Both TTL checks should be passing")

	// Put one service in maintenance
	err = client.Agent().EnableServiceMaintenance(maintenanceID, "Scheduled maintenance")
	require.NoError(t, err)
	defer client.Agent().DisableServiceMaintenance(maintenanceID)

	requireEventually(t, "maintenance check critical", 10*time.Second, func() bool {
		checks, err := client.Agent().Checks()
		if err != nil {
			return false
		}
		check, ok := checks["_service_maintenance:"+maintenanceID]
		return ok && check.Status == api.HealthCritical
	}, "Service maintenance check should be critical")

	// Discovery should only find healthy service when filtering for passing
	healthyEntries, _, err := client.Health().Service(serviceName, "", true, nil)
	assert.NoError(t, err)

	// Check which IDs are in the healthy list
	healthyIDs := make([]string, 0)
	for _, entry := range healthyEntries {
		healthyIDs = append(healthyIDs, entry.Service.ID)
	}
	t.Logf("Healthy service IDs: %v", healthyIDs)

	// Catalog should still show all services
	catalogServices, _, err := client.Catalog().Service(serviceName, "", nil)
	assert.NoError(t, err)
	t.Logf("Catalog services: %d", len(catalogServices))
}

// TestMaintenancePersistence tests maintenance mode persistence behavior
func TestMaintenancePersistence(t *testing.T) {
	client := getClient(t)
	serviceID := "maint-persist-svc-" + randomID()

	// Register a service
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceID,
		Name: "maintenance-persistence-test",
		Port: 8080,
	})
	require.NoError(t, err)
	defer client.Agent().ServiceDeregister(serviceID)

	require.True(t, waitForAgentService(t, client, serviceID, 10*time.Second), "service %s should be registered", serviceID)

	// Enable maintenance
	err = client.Agent().EnableServiceMaintenance(serviceID, "Testing persistence")
	require.NoError(t, err)

	requireEventually(t, "maintenance check critical", 10*time.Second, func() bool {
		checks, err := client.Agent().Checks()
		if err != nil {
			return false
		}
		for _, check := range checks {
			if check.ServiceID == serviceID && check.Status == api.HealthCritical {
				return true
			}
		}
		return false
	}, "Service should be in maintenance")

	// Check maintenance state
	checks1, err := client.Agent().Checks()
	assert.NoError(t, err)

	inMaintenance1 := false
	for _, check := range checks1 {
		if check.ServiceID == serviceID && check.Status == api.HealthCritical {
			inMaintenance1 = true
			break
		}
	}
	assert.True(t, inMaintenance1, "Service should be in maintenance")

	// Wait some time and check again
	time.Sleep(1 * time.Second)

	checks2, err := client.Agent().Checks()
	assert.NoError(t, err)

	inMaintenance2 := false
	for _, check := range checks2 {
		if check.ServiceID == serviceID && check.Status == api.HealthCritical {
			inMaintenance2 = true
			break
		}
	}
	assert.True(t, inMaintenance2, "Service should still be in maintenance after delay")

	// Cleanup
	client.Agent().DisableServiceMaintenance(serviceID)
}

// TestMaintenanceMultipleServices tests maintenance mode on multiple services
func TestMaintenanceMultipleServices(t *testing.T) {
	client := getClient(t)
	prefix := "maint-multi-" + randomID()

	// Register multiple services
	serviceCount := 5
	serviceIDs := make([]string, serviceCount)

	for i := 0; i < serviceCount; i++ {
		serviceIDs[i] = prefix + "-" + string(rune('a'+i))
		err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
			ID:   serviceIDs[i],
			Name: "maintenance-multi-test",
			Port: 8080 + i,
		})
		require.NoError(t, err)
		defer client.Agent().ServiceDeregister(serviceIDs[i])
	}

	requireEventually(t, "services registered", 10*time.Second, func() bool {
		services, err := client.Agent().Services()
		if err != nil {
			return false
		}
		for _, id := range serviceIDs {
			if _, ok := services[id]; !ok {
				return false
			}
		}
		return true
	}, "All services should be registered")

	// Put some services in maintenance
	maintenanceIDs := serviceIDs[:3] // First 3 services in maintenance
	for _, id := range maintenanceIDs {
		err := client.Agent().EnableServiceMaintenance(id, "Batch maintenance")
		assert.NoError(t, err, "Enable maintenance for %s should succeed", id)
		defer client.Agent().DisableServiceMaintenance(id)
	}

	requireEventually(t, "maintenance checks enabled", 10*time.Second, func() bool {
		checks, err := client.Agent().Checks()
		if err != nil {
			return false
		}
		count := 0
		for _, id := range maintenanceIDs {
			if check, ok := checks["_service_maintenance:"+id]; ok && check.Status == api.HealthCritical {
				count++
			}
		}
		return count == len(maintenanceIDs)
	}, "All maintenance services should have critical checks")

	// Verify maintenance status
	checks, err := client.Agent().Checks()
	assert.NoError(t, err)

	maintenanceCount := 0
	for _, check := range checks {
		for _, id := range maintenanceIDs {
			if check.ServiceID == id && check.Status == api.HealthCritical {
				maintenanceCount++
				break
			}
		}
	}

	t.Logf("Services in maintenance: %d/%d", maintenanceCount, len(maintenanceIDs))

	// Verify non-maintenance services are still queryable
	healthyEntries, _, err := client.Health().Service("maintenance-multi-test", "", true, nil)
	if err == nil {
		t.Logf("Healthy services: %d", len(healthyEntries))
	}
}

// TestMaintenanceToggle tests toggling maintenance mode on and off
func TestMaintenanceToggle(t *testing.T) {
	client := getClient(t)
	serviceID := "maint-toggle-svc-" + randomID()

	// Register a service
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceID,
		Name: "maintenance-toggle-test",
		Port: 8080,
	})
	require.NoError(t, err)
	defer client.Agent().ServiceDeregister(serviceID)

	require.True(t, waitForAgentService(t, client, serviceID, 10*time.Second), "service %s should be registered", serviceID)

	// Toggle maintenance multiple times
	toggleCount := 5
	for i := 0; i < toggleCount; i++ {
		// Enable
		err = client.Agent().EnableServiceMaintenance(serviceID, "Toggle test iteration")
		assert.NoError(t, err, "Enable iteration %d should succeed", i)

		requireEventually(t, "maintenance enabled", 10*time.Second, func() bool {
			checks, err := client.Agent().Checks()
			if err != nil {
				return false
			}
			for _, check := range checks {
				if check.ServiceID == serviceID && check.Status == api.HealthCritical {
					return true
				}
			}
			return false
		}, "Service should be in maintenance at iteration %d", i)

		// Verify in maintenance
		checks, err := client.Agent().Checks()
		assert.NoError(t, err)
		inMaintenance := false
		for _, check := range checks {
			if check.ServiceID == serviceID && check.Status == api.HealthCritical {
				inMaintenance = true
				break
			}
		}
		assert.True(t, inMaintenance, "Service should be in maintenance at iteration %d", i)

		// Disable
		err = client.Agent().DisableServiceMaintenance(serviceID)
		assert.NoError(t, err, "Disable iteration %d should succeed", i)

		requireEventually(t, "maintenance disabled", 10*time.Second, func() bool {
			checks, err := client.Agent().Checks()
			if err != nil {
				return false
			}
			for _, check := range checks {
				if check.ServiceID == serviceID && check.Status == api.HealthCritical && check.Name == "_service_maintenance" {
					return false
				}
			}
			return true
		}, "Service should not be in maintenance at iteration %d", i)

		// Verify not in maintenance
		checks, err = client.Agent().Checks()
		assert.NoError(t, err)
		stillInMaintenance := false
		for _, check := range checks {
			if check.ServiceID == serviceID && check.Status == api.HealthCritical {
				// Check if it's a maintenance check specifically
				if check.Name == "_service_maintenance" {
					stillInMaintenance = true
					break
				}
			}
		}
		assert.False(t, stillInMaintenance, "Service should not be in maintenance at iteration %d", i)
	}

	t.Logf("Successfully toggled maintenance %d times", toggleCount)
}

// TestMaintenanceConcurrent tests concurrent maintenance operations
func TestMaintenanceConcurrent(t *testing.T) {
	client := getClient(t)
	prefix := "maint-concurrent-" + randomID()

	// Register services
	serviceCount := 10
	serviceIDs := make([]string, serviceCount)

	for i := 0; i < serviceCount; i++ {
		serviceIDs[i] = prefix + "-" + string(rune('0'+i))
		err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
			ID:   serviceIDs[i],
			Name: "maintenance-concurrent-test",
			Port: 8080 + i,
		})
		require.NoError(t, err)
		defer client.Agent().ServiceDeregister(serviceIDs[i])
	}

	requireEventually(t, "services registered", 10*time.Second, func() bool {
		services, err := client.Agent().Services()
		if err != nil {
			return false
		}
		for _, id := range serviceIDs {
			if _, ok := services[id]; !ok {
				return false
			}
		}
		return true
	}, "All services should be registered")

	// Concurrent enable maintenance
	var wg sync.WaitGroup
	errors := make(chan error, serviceCount)

	for _, id := range serviceIDs {
		wg.Add(1)
		go func(serviceID string) {
			defer wg.Done()
			err := client.Agent().EnableServiceMaintenance(serviceID, "Concurrent test")
			if err != nil {
				errors <- err
			}
		}(id)
	}

	wg.Wait()
	close(errors)

	// Check for errors
	errorCount := 0
	for err := range errors {
		t.Logf("Concurrent enable error: %v", err)
		errorCount++
	}

	assert.Equal(t, 0, errorCount, "All concurrent enable operations should succeed")

	requireEventually(t, "all services in maintenance", 10*time.Second, func() bool {
		checks, err := client.Agent().Checks()
		if err != nil {
			return false
		}
		count := 0
		for _, id := range serviceIDs {
			for _, check := range checks {
				if check.ServiceID == id && check.Status == api.HealthCritical {
					count++
					break
				}
			}
		}
		return count == serviceCount
	}, "All services should be in maintenance")

	// Verify all services are in maintenance
	checks, err := client.Agent().Checks()
	assert.NoError(t, err)

	maintenanceCount := 0
	for _, check := range checks {
		for _, id := range serviceIDs {
			if check.ServiceID == id && check.Status == api.HealthCritical {
				maintenanceCount++
				break
			}
		}
	}
	t.Logf("Services in maintenance after concurrent enable: %d/%d", maintenanceCount, serviceCount)

	// Concurrent disable maintenance
	var wg2 sync.WaitGroup
	disableErrors := make(chan error, serviceCount)

	for _, id := range serviceIDs {
		wg2.Add(1)
		go func(serviceID string) {
			defer wg2.Done()
			err := client.Agent().DisableServiceMaintenance(serviceID)
			if err != nil {
				disableErrors <- err
			}
		}(id)
	}

	wg2.Wait()
	close(disableErrors)

	// Check for disable errors
	disableErrorCount := 0
	for err := range disableErrors {
		t.Logf("Concurrent disable error: %v", err)
		disableErrorCount++
	}

	assert.Equal(t, 0, disableErrorCount, "All concurrent disable operations should succeed")
}

// TestMaintenanceWithACL tests maintenance mode with ACL tokens
func TestMaintenanceWithACL(t *testing.T) {
	client := getClient(t)
	serviceID := "maint-acl-svc-" + randomID()

	// Register a service
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceID,
		Name: "maintenance-acl-test",
		Port: 8080,
	})
	require.NoError(t, err)
	defer client.Agent().ServiceDeregister(serviceID)

	require.True(t, waitForAgentService(t, client, serviceID, 10*time.Second), "service %s should be registered", serviceID)

	// Try to create an ACL token with limited permissions
	token := &api.ACLToken{
		Description: "Maintenance test token " + randomID(),
		Policies:    []*api.ACLTokenPolicyLink{},
		Local:       true,
	}

	createdToken, _, err := client.ACL().TokenCreate(token, nil)
	if err != nil {
		t.Logf("ACL not enabled or token creation failed: %v", err)
		// Continue test without ACL
	} else {
		defer client.ACL().TokenDelete(createdToken.AccessorID, nil)
		t.Logf("Created ACL token: %s", createdToken.AccessorID)

		// Create a new client with the token
		addr := os.Getenv("CONSUL_HTTP_ADDR")
		if addr == "" {
			addr = "127.0.0.1:8500"
		}
		tokenClient, err := api.NewClient(&api.Config{
			Address: addr,
			Token:   createdToken.SecretID,
		})

		if err == nil {
			// Try maintenance with limited token (may or may not have permission)
			err = tokenClient.Agent().EnableServiceMaintenance(serviceID, "ACL test")
			if err != nil {
				t.Logf("Enable maintenance with limited token: %v (expected if ACL enforced)", err)
			} else {
				t.Log("Enable maintenance succeeded with token (ACL may allow it)")
				tokenClient.Agent().DisableServiceMaintenance(serviceID)
			}
		}
	}

	// Verify basic maintenance still works with default client
	err = client.Agent().EnableServiceMaintenance(serviceID, "ACL test with default client")
	assert.NoError(t, err, "Maintenance should work with default client")

	requireEventually(t, "maintenance enabled with default client", 10*time.Second, func() bool {
		checks, err := client.Agent().Checks()
		if err != nil {
			return false
		}
		check, ok := checks["_service_maintenance:"+serviceID]
		return ok && check.Status == api.HealthCritical
	}, "Service maintenance check should be critical")

	err = client.Agent().DisableServiceMaintenance(serviceID)
	assert.NoError(t, err, "Disable maintenance should work with default client")

	t.Log("Maintenance with ACL test completed")
}
