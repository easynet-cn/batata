package tests

import (
	"testing"
	"time"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/require"
)

// pollFunc polls a condition until it returns true or timeout is reached.
// interval controls the polling frequency. Returns false if timed out.
func pollFunc(name string, timeout, interval time.Duration, cond func() bool) bool {
	deadline := time.Now().Add(timeout)
	for time.Now().Before(deadline) {
		if cond() {
			return true
		}
		time.Sleep(interval)
	}
	return cond() // final check
}

// waitForKV polls a client until the key is visible or timeout.
func waitForKV(t *testing.T, client *api.Client, key string, timeout time.Duration) *api.KVPair {
	var result *api.KVPair
	ok := pollFunc("kv visible", timeout, 100*time.Millisecond, func() bool {
		pair, _, err := client.KV().Get(key, nil)
		if err == nil && pair != nil {
			result = pair
			return true
		}
		return false
	})
	if !ok {
		return nil
	}
	return result
}

// waitForKVAbsent polls a client until the key is gone or timeout.
func waitForKVAbsent(t *testing.T, client *api.Client, key string, timeout time.Duration) bool {
	return pollFunc("kv absent", timeout, 100*time.Millisecond, func() bool {
		pair, _, err := client.KV().Get(key, nil)
		return err == nil && pair == nil
	})
}

// waitForSession polls a client until the session is visible or timeout.
func waitForSession(t *testing.T, client *api.Client, sessionID string, timeout time.Duration) *api.SessionEntry {
	var result *api.SessionEntry
	ok := pollFunc("session visible", timeout, 100*time.Millisecond, func() bool {
		info, _, err := client.Session().Info(sessionID, nil)
		if err == nil && info != nil {
			result = info
			return true
		}
		return false
	})
	if !ok {
		return nil
	}
	return result
}

// waitForSessionAbsent polls until the session is gone or timeout.
func waitForSessionAbsent(t *testing.T, client *api.Client, sessionID string, timeout time.Duration) bool {
	return pollFunc("session absent", timeout, 100*time.Millisecond, func() bool {
		info, _, err := client.Session().Info(sessionID, nil)
		return err == nil && info == nil
	})
}

// waitForService polls a client until the service appears in catalog or timeout.
func waitForService(t *testing.T, client *api.Client, serviceName string, timeout time.Duration) bool {
	return pollFunc("service visible", timeout, 100*time.Millisecond, func() bool {
		services, _, err := client.Catalog().Services(nil)
		if err != nil {
			return false
		}
		_, ok := services[serviceName]
		return ok
	})
}

// waitForServiceAbsent polls until the service is gone from catalog or timeout.
func waitForServiceAbsent(t *testing.T, client *api.Client, serviceName string, timeout time.Duration) bool {
	return pollFunc("service absent", timeout, 100*time.Millisecond, func() bool {
		services, _, err := client.Catalog().Services(nil)
		if err != nil {
			return false
		}
		_, ok := services[serviceName]
		return !ok
	})
}

// waitForAgentService polls the agent until the service appears or timeout.
func waitForAgentService(t *testing.T, client *api.Client, serviceID string, timeout time.Duration) bool {
	return pollFunc("agent service visible", timeout, 100*time.Millisecond, func() bool {
		services, err := client.Agent().Services()
		if err != nil {
			return false
		}
		_, ok := services[serviceID]
		return ok
	})
}

// waitForAgentServiceAbsent polls until the service is gone from agent or timeout.
func waitForAgentServiceAbsent(t *testing.T, client *api.Client, serviceID string, timeout time.Duration) bool {
	return pollFunc("agent service absent", timeout, 100*time.Millisecond, func() bool {
		services, err := client.Agent().Services()
		if err != nil {
			return false
		}
		_, ok := services[serviceID]
		return !ok
	})
}

// waitForConfigEntry polls until the config entry is visible or timeout.
func waitForConfigEntry(t *testing.T, client *api.Client, kind string, name string, timeout time.Duration) bool {
	return pollFunc("config entry visible", timeout, 100*time.Millisecond, func() bool {
		_, _, err := client.ConfigEntries().Get(kind, name, nil)
		return err == nil
	})
}

// waitForConfigEntryAbsent polls until the config entry is gone or timeout.
func waitForConfigEntryAbsent(t *testing.T, client *api.Client, kind string, name string, timeout time.Duration) bool {
	return pollFunc("config entry absent", timeout, 100*time.Millisecond, func() bool {
		_, _, err := client.ConfigEntries().Get(kind, name, nil)
		return err != nil
	})
}

// waitForHealthChecks polls until the service has health checks or timeout.
func waitForHealthChecks(t *testing.T, client *api.Client, serviceName string, timeout time.Duration) []*api.HealthCheck {
	var result []*api.HealthCheck
	ok := pollFunc("health checks visible", timeout, 200*time.Millisecond, func() bool {
		checks, _, err := client.Health().Checks(serviceName, nil)
		if err == nil && len(checks) > 0 {
			result = checks
			return true
		}
		return false
	})
	if !ok {
		return nil
	}
	return result
}

// requireEventually asserts that a condition becomes true within timeout.
func requireEventually(t *testing.T, name string, timeout time.Duration, cond func() bool, msgAndArgs ...interface{}) {
	require.True(t, pollFunc(name, timeout, 100*time.Millisecond, cond), msgAndArgs...)
}
