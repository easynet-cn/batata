package tests

// Tests for active health check execution (HTTP/TCP) and DeregisterCriticalServiceAfter.
// These validate the reactor actually executes checks and auto-deregisters services.

import (
	"fmt"
	"testing"
	"time"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// ==================== Active Health Check Execution Tests ====================

// HCE-001: TCP health check executes and transitions to critical for unreachable host
func TestHealthCheckTCPExecutes(t *testing.T) {
	client := getClient(t)
	agent := client.Agent()
	serviceID := "hce001-tcp-exec-" + randomID()

	// Register service with TCP check pointing to unreachable port
	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID:      serviceID,
		Name:    "tcp-exec-svc",
		Port:    19999,
		Address: "127.0.0.1",
		Check: &api.AgentServiceCheck{
			TCP:      "127.0.0.1:19", // Port 19 — typically not listening
			Interval: "2s",
			Timeout:  "1s",
		},
	})
	require.NoError(t, err, "Service registration should succeed")
	defer agent.ServiceDeregister(serviceID)

	// Wait for at least one check execution cycle
	time.Sleep(5 * time.Second)

	// Verify check status transitioned to critical
	checks, err := agent.Checks()
	require.NoError(t, err)

	checkID := "service:" + serviceID
	check, ok := checks[checkID]
	require.True(t, ok, "Check %s should exist in agent checks", checkID)
	assert.Equal(t, "critical", check.Status,
		"TCP check to unreachable port should be critical after execution")
	assert.NotEmpty(t, check.Output,
		"Check output should contain failure details")
	assert.Contains(t, check.Output, "TCP",
		"Output should indicate TCP check type")
}

// HCE-002: HTTP health check executes and transitions to critical for unreachable host
func TestHealthCheckHTTPExecutes(t *testing.T) {
	client := getClient(t)
	agent := client.Agent()
	serviceID := "hce002-http-exec-" + randomID()

	// Register service with HTTP check pointing to unreachable endpoint
	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID:      serviceID,
		Name:    "http-exec-svc",
		Port:    19998,
		Address: "127.0.0.1",
		Check: &api.AgentServiceCheck{
			HTTP:     "http://127.0.0.1:19/health", // Port 19 — not an HTTP server
			Interval: "2s",
			Timeout:  "1s",
		},
	})
	require.NoError(t, err, "Service registration should succeed")
	defer agent.ServiceDeregister(serviceID)

	// Wait for check execution
	time.Sleep(5 * time.Second)

	// Verify via health API
	entries, _, err := client.Health().Service("http-exec-svc", "", false, nil)
	require.NoError(t, err)

	found := false
	for _, entry := range entries {
		if entry.Service.ID == serviceID {
			found = true
			// Should have at least one check in critical state
			hasCritical := false
			for _, check := range entry.Checks {
				if check.ServiceID == serviceID && check.Status == "critical" {
					hasCritical = true
					assert.NotEmpty(t, check.Output,
						"Critical check should have output describing failure")
				}
			}
			assert.True(t, hasCritical,
				"HTTP check to unreachable host should have critical status")
		}
	}
	assert.True(t, found, "Service should be found in health query")
}

// HCE-003: TCP check to reachable port stays passing
func TestHealthCheckTCPReachableStaysPassing(t *testing.T) {
	client := getClient(t)
	agent := client.Agent()
	serviceID := "hce003-tcp-pass-" + randomID()

	// Use the Batata server's own port (8500/8848) as the reachable target
	addr := "127.0.0.1:8500"

	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID:      serviceID,
		Name:    "tcp-pass-svc",
		Port:    8500,
		Address: "127.0.0.1",
		Check: &api.AgentServiceCheck{
			TCP:      addr,
			Interval: "2s",
			Timeout:  "1s",
			Status:   "critical", // Start critical, should transition to passing
		},
	})
	require.NoError(t, err, "Service registration should succeed")
	defer agent.ServiceDeregister(serviceID)

	// Wait for check execution
	time.Sleep(5 * time.Second)

	checks, err := agent.Checks()
	require.NoError(t, err)

	checkID := "service:" + serviceID
	check, ok := checks[checkID]
	require.True(t, ok, "Check should exist")
	assert.Equal(t, "passing", check.Status,
		"TCP check to reachable port should be passing")
}

// HCE-004: Health API /v1/health/service only_passing filter works with active checks
func TestHealthCheckOnlyPassingFilter(t *testing.T) {
	client := getClient(t)
	agent := client.Agent()

	passingID := "hce004-passing-" + randomID()
	failingID := "hce004-failing-" + randomID()
	svcName := "hce004-filter-svc-" + randomID()

	// Register passing service (TTL, manually set passing)
	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID:   passingID,
		Name: svcName,
		Port: 8080,
		Check: &api.AgentServiceCheck{
			CheckID: "service:" + passingID,
			TTL:     "30s",
			Status:  "passing",
		},
	})
	require.NoError(t, err)
	defer agent.ServiceDeregister(passingID)

	// Register failing service (TCP to unreachable)
	err = agent.ServiceRegister(&api.AgentServiceRegistration{
		ID:   failingID,
		Name: svcName,
		Port: 8081,
		Check: &api.AgentServiceCheck{
			TCP:      "127.0.0.1:19",
			Interval: "2s",
			Timeout:  "1s",
		},
	})
	require.NoError(t, err)
	defer agent.ServiceDeregister(failingID)

	// Wait for TCP check to fail
	time.Sleep(5 * time.Second)

	// Query only passing
	entries, _, err := client.Health().Service(svcName, "", true, nil)
	require.NoError(t, err)

	// Should only contain the passing instance
	for _, entry := range entries {
		assert.NotEqual(t, failingID, entry.Service.ID,
			"Failing service should NOT appear in only_passing=true query")
	}
}

// ==================== DeregisterCriticalServiceAfter Tests ====================

// HCE-005: Service auto-deregisters after critical timeout
func TestDeregisterCriticalServiceAfter(t *testing.T) {
	client := getClient(t)
	agent := client.Agent()
	serviceID := "hce005-deregister-" + randomID()

	// Register service with TCP check + short deregister timeout
	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID:      serviceID,
		Name:    "deregister-svc",
		Port:    19997,
		Address: "127.0.0.1",
		Check: &api.AgentServiceCheck{
			TCP:                            "127.0.0.1:19", // Unreachable
			Interval:                       "1s",
			Timeout:                        "1s",
			DeregisterCriticalServiceAfter: "10s",
		},
	})
	require.NoError(t, err, "Service registration should succeed")

	// Verify service exists initially
	services, err := agent.Services()
	require.NoError(t, err)
	_, exists := services[serviceID]
	require.True(t, exists, "Service should exist initially after registration")

	// Wait for check to go critical + deregister timeout
	// 1s interval + 1s timeout + 10s deregister + buffer
	time.Sleep(20 * time.Second)

	// Verify service was auto-deregistered
	services, err = agent.Services()
	require.NoError(t, err)
	_, stillExists := services[serviceID]
	assert.False(t, stillExists,
		"Service should be auto-deregistered after DeregisterCriticalServiceAfter timeout")
}

// HCE-006: DeregisterCriticalServiceAfter does NOT deregister if check recovers
func TestDeregisterCriticalRecovery(t *testing.T) {
	client := getClient(t)
	agent := client.Agent()
	serviceID := "hce006-recovery-" + randomID()
	checkID := "service:" + serviceID

	// Register with TTL check + deregister timeout (use TTL so we can manually control status)
	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceID,
		Name: "recovery-svc",
		Port: 8080,
		Check: &api.AgentServiceCheck{
			CheckID:                        checkID,
			TTL:                            "30s",
			Status:                         "passing",
			DeregisterCriticalServiceAfter: "15s",
		},
	})
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceID)

	// Fail the check
	err = agent.FailTTL(checkID, "temporarily down")
	require.NoError(t, err)

	// Wait some time (less than deregister timeout)
	time.Sleep(3 * time.Second)

	// Recover the check before timeout
	err = agent.PassTTL(checkID, "recovered")
	require.NoError(t, err)

	// Wait past the original deregister timeout
	time.Sleep(15 * time.Second)

	// Service should still exist (recovered before timeout)
	services, err := agent.Services()
	require.NoError(t, err)
	_, exists := services[serviceID]
	assert.True(t, exists,
		"Service should NOT be deregistered if check recovered before timeout")

	// Check should be passing
	checks, err := agent.Checks()
	require.NoError(t, err)
	if c, ok := checks[checkID]; ok {
		assert.Equal(t, "passing", c.Status,
			"Check should be passing after recovery")
	}
}

// HCE-007: Multiple services with different deregister timeouts
func TestDeregisterCriticalMultipleServices(t *testing.T) {
	client := getClient(t)
	agent := client.Agent()

	shortID := "hce007-short-" + randomID()
	longID := "hce007-long-" + randomID()

	// Short timeout service (5s)
	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID:   shortID,
		Name: "short-timeout-svc",
		Port: 19996,
		Check: &api.AgentServiceCheck{
			TCP:                            "127.0.0.1:19",
			Interval:                       "1s",
			Timeout:                        "1s",
			DeregisterCriticalServiceAfter: "5s",
		},
	})
	require.NoError(t, err)

	// Long timeout service (60s)
	err = agent.ServiceRegister(&api.AgentServiceRegistration{
		ID:   longID,
		Name: "long-timeout-svc",
		Port: 19995,
		Check: &api.AgentServiceCheck{
			TCP:                            "127.0.0.1:19",
			Interval:                       "1s",
			Timeout:                        "1s",
			DeregisterCriticalServiceAfter: "60s",
		},
	})
	require.NoError(t, err)
	defer agent.ServiceDeregister(longID) // cleanup long-timeout one

	// Wait for short timeout to trigger but not long
	time.Sleep(15 * time.Second)

	services, err := agent.Services()
	require.NoError(t, err)

	_, shortExists := services[shortID]
	_, longExists := services[longID]

	assert.False(t, shortExists,
		"Short-timeout service should be deregistered after 5s critical")
	assert.True(t, longExists,
		"Long-timeout service should still exist (60s timeout not reached)")
}

// HCE-008: Verify check output contains useful diagnostic information
func TestHealthCheckOutputDiagnostics(t *testing.T) {
	client := getClient(t)
	agent := client.Agent()
	serviceID := "hce008-diag-" + randomID()

	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceID,
		Name: "diag-svc",
		Port: 19994,
		Check: &api.AgentServiceCheck{
			TCP:      "127.0.0.1:19",
			Interval: "2s",
			Timeout:  "1s",
		},
	})
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceID)

	time.Sleep(5 * time.Second)

	checks, err := agent.Checks()
	require.NoError(t, err)

	checkID := "service:" + serviceID
	check, ok := checks[checkID]
	require.True(t, ok, "Check should exist")

	assert.Equal(t, "critical", check.Status)
	assert.NotEmpty(t, check.Output,
		"Output must contain diagnostic info")

	// Output should mention the target address or failure reason
	outputHasInfo := false
	for _, keyword := range []string{"TCP", "127.0.0.1", "failed", "timeout", "connection"} {
		if assert.ObjectsAreEqual(true, containsStr(check.Output, keyword)) {
			outputHasInfo = true
			break
		}
	}
	assert.True(t, outputHasInfo,
		fmt.Sprintf("Output should contain diagnostic info, got: %s", check.Output))
}

func containsStr(s, substr string) bool {
	return len(s) >= len(substr) && (s == substr || len(s) > 0 && containsSubstring(s, substr))
}

func containsSubstring(s, sub string) bool {
	for i := 0; i <= len(s)-len(sub); i++ {
		if s[i:i+len(sub)] == sub {
			return true
		}
	}
	return false
}
