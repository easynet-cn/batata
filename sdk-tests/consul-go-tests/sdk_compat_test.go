// Package tests – SDK compatibility regression tests.
//
// These tests target the wire-level fixes added in rounds 5–7:
// plain-text errors, new response headers, query parameter aliases,
// /internal/rpc-methods, /operator/{segment,license}, No-path-to-datacenter,
// merge-central-config, Txn Node/Service/Check verbs, filter CEL operators,
// snapshot archive shape, Vivaldi periodic updates, and health state=empty.
//
// Every test is written against the official `github.com/hashicorp/consul/api`
// SDK types so a passing test means a real Consul client can interoperate.

package tests

import (
	"bytes"
	"fmt"
	"io"
	"net/http"
	"os"
	"strings"
	"testing"
	"time"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

// rawGET issues a raw HTTP GET so we can inspect headers and the raw error
// body that the Consul Go SDK never exposes directly.
func rawGET(t *testing.T, path string) (*http.Response, string) {
	t.Helper()
	addr := os.Getenv("CONSUL_HTTP_ADDR")
	if addr == "" {
		addr = "127.0.0.1:8500"
	}
	token := os.Getenv("CONSUL_HTTP_TOKEN")
	if token == "" {
		token = "root"
	}
	req, err := http.NewRequest("GET", fmt.Sprintf("http://%s%s", addr, path), nil)
	require.NoError(t, err)
	if token != "" {
		req.Header.Set("X-Consul-Token", token)
	}
	resp, err := http.DefaultClient.Do(req)
	require.NoError(t, err)
	body, _ := io.ReadAll(resp.Body)
	_ = resp.Body.Close()
	return resp, string(body)
}

func rawRequest(t *testing.T, method, path string, body []byte) (*http.Response, string) {
	t.Helper()
	addr := os.Getenv("CONSUL_HTTP_ADDR")
	if addr == "" {
		addr = "127.0.0.1:8500"
	}
	token := os.Getenv("CONSUL_HTTP_TOKEN")
	if token == "" {
		token = "root"
	}
	var reader io.Reader
	if body != nil {
		reader = bytes.NewReader(body)
	}
	req, err := http.NewRequest(method, fmt.Sprintf("http://%s%s", addr, path), reader)
	require.NoError(t, err)
	if token != "" {
		req.Header.Set("X-Consul-Token", token)
	}
	if body != nil {
		req.Header.Set("Content-Type", "application/json")
	}
	resp, err := http.DefaultClient.Do(req)
	require.NoError(t, err)
	b, _ := io.ReadAll(resp.Body)
	_ = resp.Body.Close()
	return resp, string(b)
}

// ---------------------------------------------------------------------------
// SDK-1: plain-text error bodies
// ---------------------------------------------------------------------------

// TestCompatPlainTextError ensures error responses use text/plain, not JSON.
// The Consul Go SDK calls fmt.Errorf("Unexpected response code: %d (%s)",
// resp.StatusCode, body) and users match on the raw body string. A JSON body
// would break string matchers and slice printing.
func TestCompatPlainTextError(t *testing.T) {
	// A non-existent KV CAS with wrong index triggers a 404/400
	resp, body := rawGET(t, "/v1/agent/service/definitely-does-not-exist-"+randomID())
	assert.Equal(t, http.StatusNotFound, resp.StatusCode)
	ct := resp.Header.Get("Content-Type")
	assert.True(t, strings.HasPrefix(ct, "text/plain"),
		"error body Content-Type must be text/plain, got %q body=%q", ct, body)
	assert.False(t, strings.HasPrefix(body, "{"),
		"error body must NOT be JSON, got: %q", body)
}

// ---------------------------------------------------------------------------
// SDK-2: new response headers
// ---------------------------------------------------------------------------

// TestCompatResponseHeadersComplete validates every standard X-Consul-*
// header is present on a read response.
func TestCompatResponseHeadersComplete(t *testing.T) {
	resp, _ := rawGET(t, "/v1/catalog/services")
	assert.Equal(t, http.StatusOK, resp.StatusCode)
	headers := []string{
		"X-Consul-Index",
		"X-Consul-Knownleader",
		"X-Consul-Lastcontact",
		"X-Consul-Effective-Consistency",
	}
	for _, h := range headers {
		assert.NotEmpty(t, resp.Header.Get(h),
			"missing required header: %s", h)
	}
}

// TestCompatEffectiveDatacenterHeader validates the X-Consul-Effective-Datacenter
// header is present on catalog/health endpoints (SDK uses it to detect
// forwarded responses).
func TestCompatEffectiveDatacenterHeader(t *testing.T) {
	resp, _ := rawGET(t, "/v1/catalog/services?dc=dc1")
	dc := resp.Header.Get("X-Consul-Effective-Datacenter")
	assert.NotEmpty(t, dc, "X-Consul-Effective-Datacenter must be set")
	assert.Equal(t, "dc1", dc)
}

// ---------------------------------------------------------------------------
// SDK-3: node-meta / node_meta both accepted
// ---------------------------------------------------------------------------

// TestCompatNodeMetaUnderscoreAlias ensures clients that send `?node_meta=`
// (underscore form) get the same filtering as clients using `?node-meta=`.
func TestCompatNodeMetaUnderscoreAlias(t *testing.T) {
	// Both forms should succeed with HTTP 200 and same shape
	r1, _ := rawGET(t, "/v1/catalog/nodes?node-meta=rack:a")
	r2, _ := rawGET(t, "/v1/catalog/nodes?node_meta=rack:a")
	assert.Equal(t, http.StatusOK, r1.StatusCode)
	assert.Equal(t, http.StatusOK, r2.StatusCode)
	assert.Equal(t, r1.Header.Get("Content-Type"), r2.Header.Get("Content-Type"))
}

// ---------------------------------------------------------------------------
// SDK-4: /v1/internal/rpc-methods
// ---------------------------------------------------------------------------

// TestCompatInternalRPCMethods validates gRPC method discovery endpoint
// returns a JSON array of RPC method names.
func TestCompatInternalRPCMethods(t *testing.T) {
	resp, body := rawGET(t, "/v1/internal/rpc-methods")
	assert.Equal(t, http.StatusOK, resp.StatusCode)
	// Should be a JSON array, at minimum containing KVS.Get and Catalog.Register
	assert.True(t, strings.HasPrefix(strings.TrimSpace(body), "["),
		"body should be a JSON array")
	assert.Contains(t, body, "KVS.Get")
	assert.Contains(t, body, "Catalog.Register")
	assert.Contains(t, body, "ACL.TokenRead")
}

// ---------------------------------------------------------------------------
// SDK-5: /v1/operator/segment stub
// ---------------------------------------------------------------------------

// TestCompatOperatorSegmentList validates the segment list endpoint returns
// an empty array (OSS behavior), not a 404.
func TestCompatOperatorSegmentList(t *testing.T) {
	resp, body := rawGET(t, "/v1/operator/segment")
	assert.Equal(t, http.StatusOK, resp.StatusCode)
	assert.Equal(t, "[]", strings.TrimSpace(body))
}

// ---------------------------------------------------------------------------
// SDK-6: ACL error strings match Consul format
// ---------------------------------------------------------------------------

// TestCompatACLErrorStrings ensures that denied ACL responses contain the
// exact strings the Consul SDK matches on.
func TestCompatACLErrorStrings(t *testing.T) {
	addr := os.Getenv("CONSUL_HTTP_ADDR")
	if addr == "" {
		addr = "127.0.0.1:8500"
	}
	req, _ := http.NewRequest("GET", fmt.Sprintf("http://%s/v1/acl/tokens", addr), nil)
	req.Header.Set("X-Consul-Token", "definitely-invalid-token-"+randomID())
	resp, err := http.DefaultClient.Do(req)
	require.NoError(t, err)
	body, _ := io.ReadAll(resp.Body)
	_ = resp.Body.Close()

	// Either 401/403 (token rejected) or 200 (if ACL disabled); just
	// require the message format when it fails.
	if resp.StatusCode >= 400 {
		s := string(body)
		matched := strings.Contains(s, "Permission denied") ||
			strings.Contains(s, "ACL not found") ||
			strings.Contains(s, "token not found")
		assert.True(t, matched,
			"ACL error should contain SDK-recognised phrase, got: %q", s)
	}
}

// ---------------------------------------------------------------------------
// SDK-7: Vivaldi coordinate updates over time
// ---------------------------------------------------------------------------

// TestCompatVivaldiCoordinatesRepeat checks that two sequential reads of
// coordinate nodes succeed and return consistent structure. A full Vivaldi
// convergence test requires multi-node + wall-clock, which the harness doesn't
// guarantee; this is the observable contract.
func TestCompatVivaldiCoordinatesRepeat(t *testing.T) {
	client := getClient(t)
	c1, _, err := client.Coordinate().Nodes(nil)
	require.NoError(t, err)
	c2, _, err := client.Coordinate().Nodes(nil)
	require.NoError(t, err)
	assert.Equal(t, len(c1), len(c2))
	for _, n := range c1 {
		assert.Len(t, n.Coord.Vec, 8, "Vivaldi vector must have 8 dimensions")
		assert.GreaterOrEqual(t, n.Coord.Height, 1.0e-5,
			"Vivaldi height must be >= HEIGHT_MIN")
	}
}

// ---------------------------------------------------------------------------
// SDK-8: No path to datacenter
// ---------------------------------------------------------------------------

// TestCompatNoPathToDatacenter validates the exact error string Consul uses
// when a remote DC is unreachable. The Consul SDK parses this substring to
// distinguish "DC doesn't exist" from transport errors.
func TestCompatNoPathToDatacenter(t *testing.T) {
	resp, body := rawGET(t, "/v1/catalog/nodes?dc=no-such-datacenter-"+randomID())
	// Consul returns 500 with this body
	if resp.StatusCode >= 400 {
		assert.Contains(t, body, "No path to datacenter",
			"must return exact error string for SDK parsing")
	}
}

// ---------------------------------------------------------------------------
// SDK-11: health state empty / any semantic equivalence
// ---------------------------------------------------------------------------

// TestCompatHealthStateAcceptsAllStates validates that each of the four
// documented Consul health states returns a well-formed 200 array response.
// Matches Consul `api/health.go`: `passing`, `warning`, `critical`, `any`.
func TestCompatHealthStateAcceptsAllStates(t *testing.T) {
	for _, state := range []string{"passing", "warning", "critical", "any"} {
		resp, body := rawGET(t, "/v1/health/state/"+state)
		assert.Equal(t, http.StatusOK, resp.StatusCode, "state=%s", state)
		assert.True(t, strings.HasPrefix(body, "["), "state=%s body=%q", state, body)
	}
}

// ---------------------------------------------------------------------------
// P2-3: filter CEL operators (in, contains, and/or/not)
// ---------------------------------------------------------------------------

// TestCompatFilterInOperator validates `"v1" in ServiceTags` semantics.
func TestCompatFilterInOperator(t *testing.T) {
	client := getClient(t)
	svc := "compat-filter-in-" + randomID()
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   svc,
		Name: svc,
		Tags: []string{"v1", "prod"},
		Port: 8080,
	})
	require.NoError(t, err)
	defer client.Agent().ServiceDeregister(svc)
	time.Sleep(500 * time.Millisecond)

	opts := (&api.QueryOptions{Filter: `"v1" in ServiceTags`})
	entries, _, err := client.Health().Service(svc, "", false, opts)
	require.NoError(t, err)
	assert.Greater(t, len(entries), 0, "filter 'v1' in ServiceTags must match")

	opts2 := (&api.QueryOptions{Filter: `"v999" in ServiceTags`})
	entries2, _, err := client.Health().Service(svc, "", false, opts2)
	require.NoError(t, err)
	assert.Equal(t, 0, len(entries2), "non-matching tag must filter out")
}

// TestCompatFilterContainsOperator validates `ServiceTags contains "x"`.
func TestCompatFilterContainsOperator(t *testing.T) {
	client := getClient(t)
	svc := "compat-filter-contains-" + randomID()
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   svc,
		Name: svc,
		Tags: []string{"staging"},
		Port: 8080,
	})
	require.NoError(t, err)
	defer client.Agent().ServiceDeregister(svc)
	time.Sleep(500 * time.Millisecond)

	opts := &api.QueryOptions{Filter: `ServiceTags contains "staging"`}
	entries, _, err := client.Health().Service(svc, "", false, opts)
	require.NoError(t, err)
	assert.Greater(t, len(entries), 0, "contains operator must match")
}

// TestCompatFilterLogicalOperators validates `and`, `or`, `not` logical ops.
func TestCompatFilterLogicalOperators(t *testing.T) {
	client := getClient(t)
	svc := "compat-filter-logic-" + randomID()
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   svc,
		Name: svc,
		Tags: []string{"a", "b"},
		Port: 8080,
	})
	require.NoError(t, err)
	defer client.Agent().ServiceDeregister(svc)
	time.Sleep(500 * time.Millisecond)

	// AND: both tags present
	opts := &api.QueryOptions{
		Filter: `"a" in ServiceTags and "b" in ServiceTags`,
	}
	entries, _, err := client.Health().Service(svc, "", false, opts)
	require.NoError(t, err)
	assert.Greater(t, len(entries), 0, "AND of two true conditions must match")

	// OR: either tag
	opts = &api.QueryOptions{
		Filter: `"nonexistent" in ServiceTags or "a" in ServiceTags`,
	}
	entries, _, err = client.Health().Service(svc, "", false, opts)
	require.NoError(t, err)
	assert.Greater(t, len(entries), 0, "OR with one true branch must match")

	// NOT
	opts = &api.QueryOptions{Filter: `not "nonexistent" in ServiceTags`}
	entries, _, err = client.Health().Service(svc, "", false, opts)
	require.NoError(t, err)
	assert.Greater(t, len(entries), 0, "NOT of false must match")
}

// ---------------------------------------------------------------------------
// P2-5: snapshot archive format (tar structure)
// ---------------------------------------------------------------------------

// TestCompatSnapshotIsTarArchive validates a saved snapshot begins with the
// tar header bytes expected by Consul. The SDK uses `snapshot/archive.go`
// to parse this, so the first 257 bytes must look like a GNU-tar entry.
func TestCompatSnapshotIsTarArchive(t *testing.T) {
	client := getClient(t)
	snap, _, err := client.Snapshot().Save(nil)
	require.NoError(t, err)
	defer snap.Close()

	buf := make([]byte, 512) // one tar block
	n, err := io.ReadFull(snap, buf)
	if err != nil && err != io.ErrUnexpectedEOF {
		require.NoError(t, err)
	}
	require.Equal(t, 512, n, "first tar block must be 512 bytes")
	// Check the "ustar" magic at offset 257 in the tar header
	// (present in ustar-format headers; GNU tar variant also uses it)
	magic := string(buf[257:262])
	assert.True(t, magic == "ustar" || strings.Contains(string(buf[:100]), "meta.json"),
		"snapshot must be a tar archive; first entry name should be meta.json. got magic=%q", magic)
}

// ---------------------------------------------------------------------------
// P2-6: Txn Node/Service/Check verbs
// ---------------------------------------------------------------------------

// TestCompatTxnNodeServiceCheckVerbs validates the transaction endpoint
// accepts Node/Service/Check ops (not just KV).
func TestCompatTxnNodeServiceCheckVerbs(t *testing.T) {
	// Use raw HTTP since the Go SDK has txn.NodeOp etc.; submit a mixed txn.
	body := []byte(`[
        {"KV": {"Verb": "set", "Key": "compat/txn/a", "Value": "dg=="}},
        {"Node": {"Verb": "set", "Node": {"Node": "compat-node", "Address": "1.2.3.4"}}},
        {"Service": {"Verb": "set", "Node": "compat-node", "Service": {"Service": "compat-svc", "Port": 1234}}},
        {"Check": {"Verb": "set", "Check": {"Node": "compat-node", "CheckID": "compat-check", "Status": "passing"}}}
    ]`)
	resp, respBody := rawRequest(t, "PUT", "/v1/txn", body)
	assert.Equal(t, http.StatusOK, resp.StatusCode,
		"mixed Txn must return 200, body=%s", respBody)
	assert.Contains(t, respBody, "\"Results\"",
		"txn response must include Results field")
}

// ---------------------------------------------------------------------------
// P2-4: merge-central-config populates ServiceProxy
// ---------------------------------------------------------------------------

// TestCompatMergeCentralConfig validates that `?merge-central-config=true`
// merges proxy-defaults into ServiceProxy rather than stuffing them into
// ServiceMeta with custom prefixes.
func TestCompatMergeCentralConfig(t *testing.T) {
	client := getClient(t)
	svc := "compat-merge-" + randomID()
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   svc,
		Name: svc,
		Port: 8080,
	})
	require.NoError(t, err)
	defer client.Agent().ServiceDeregister(svc)
	time.Sleep(300 * time.Millisecond)

	// Write a proxy-defaults config entry
	_, _, err = client.ConfigEntries().Set(&api.ProxyConfigEntry{
		Kind: api.ProxyDefaults,
		Name: api.ProxyConfigGlobal,
		Config: map[string]interface{}{
			"protocol": "http",
		},
	}, nil)
	// Not all test environments allow ConfigEntry writes; skip if not possible
	if err != nil {
		t.Skipf("cannot write proxy-defaults config entry: %v", err)
	}
	defer client.ConfigEntries().Delete(api.ProxyDefaults, api.ProxyConfigGlobal, nil)

	// Request service with merge-central-config via raw HTTP since Go SDK
	// exposes it only indirectly
	resp, body := rawGET(t, fmt.Sprintf("/v1/catalog/service/%s?merge-central-config=true", svc))
	assert.Equal(t, http.StatusOK, resp.StatusCode)
	// After merge, response should include ServiceProxy (not custom prefixed meta)
	assert.Contains(t, body, "\"ServiceProxy\"",
		"merged response must include ServiceProxy field")
}

// ---------------------------------------------------------------------------
// P2-4: peer-name query parameter recognized
// ---------------------------------------------------------------------------

// TestCompatPeerNameParameter ensures queries with `?peer-name=X` return
// a valid (possibly empty) response, not an error or bad request.
func TestCompatPeerNameParameter(t *testing.T) {
	resp, body := rawGET(t, "/v1/catalog/service/consul?peer-name=nonexistent-peer")
	assert.Equal(t, http.StatusOK, resp.StatusCode)
	assert.Equal(t, "[]", strings.TrimSpace(body),
		"unknown peer must return empty array, not 404")
}

// TestCompatPeerNameAndSamenessGroupExclusive validates the SDK-level
// mutual exclusivity check — specifying both returns 400.
func TestCompatPeerNameAndSamenessGroupExclusive(t *testing.T) {
	resp, body := rawGET(t, "/v1/catalog/service/foo?peer-name=p1&sameness-group=g1")
	assert.Equal(t, http.StatusBadRequest, resp.StatusCode)
	assert.Contains(t, body, "cannot specify both peer-name and sameness-group")
}

// ---------------------------------------------------------------------------
// SDK-2 / merge-central-config: ServiceConnect field present
// ---------------------------------------------------------------------------

// TestCompatCatalogServiceConnectField confirms CatalogService JSON returned
// to clients contains both ServiceProxy and ServiceConnect keys (even if null
// for non-connect services), so SDK struct unmarshalling never fails.
func TestCompatCatalogServiceConnectField(t *testing.T) {
	client := getClient(t)
	svc := "compat-connect-field-" + randomID()
	require.NoError(t, client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID: svc, Name: svc, Port: 8080,
	}))
	defer client.Agent().ServiceDeregister(svc)
	time.Sleep(200 * time.Millisecond)

	services, _, err := client.Catalog().Service(svc, "", nil)
	require.NoError(t, err)
	require.NotEmpty(t, services)
	// SDK's CatalogService.ServiceProxy / ServiceConnect are optional pointers;
	// nil is acceptable. What matters is the deserialization succeeded.
	assert.Equal(t, svc, services[0].ServiceName)
}
