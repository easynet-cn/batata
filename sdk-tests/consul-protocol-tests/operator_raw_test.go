package tests

import (
	"net/http"
	"strings"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// TestOperatorUtilizationEnterpriseOnly checks the OSS-style Enterprise error
// on the raw /v1/operator/utilization endpoint. The Consul Go SDK exposes no
// public Utilization() method, so this is a protocol assertion by design.
func TestOperatorUtilizationEnterpriseOnly(t *testing.T) {
	resp, body := rawRequest(t, http.MethodPut, "/v1/operator/utilization", nil)
	require.Equal(t, 501, resp.StatusCode, "utilization must return 501 on OSS/Batata")
	assert.True(t, strings.Contains(body, "Enterprise"),
		"body should mention Enterprise, got: %s", body)
}
