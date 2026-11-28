package nacos_go_tests

// Shared harness for the official Nacos Go SDK compatibility suite.
//
// Every case talks to a real Batata server through the public SDK API; none of
// the SDK internals are mocked. The server address and credentials are read
// from NACOS_SERVER / NACOS_USERNAME / NACOS_PASSWORD so the same binary runs
// unchanged in the six SDK matrix combinations.

import (
	"crypto/md5"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/url"
	"os"
	"strconv"
	"strings"
	"sync/atomic"
	"testing"
	"time"

	"github.com/nacos-group/nacos-sdk-go/v2/clients"
	"github.com/nacos-group/nacos-sdk-go/v2/clients/config_client"
	"github.com/nacos-group/nacos-sdk-go/v2/clients/naming_client"
	"github.com/nacos-group/nacos-sdk-go/v2/common/constant"
	"github.com/nacos-group/nacos-sdk-go/v2/vo"
)

const defaultGroup = "DEFAULT_GROUP"

var caseSeq atomic.Uint64

func uniqueID(prefix string) string {
	return fmt.Sprintf("ngo-%s-%d-%d", prefix, time.Now().UnixNano(), caseSeq.Add(1))
}

func envOr(key, fallback string) string {
	if v := strings.TrimSpace(os.Getenv(key)); v != "" {
		return v
	}
	return fallback
}

func serverAddr() string {
	addr := strings.TrimPrefix(envOr("NACOS_SERVER", "127.0.0.1:8848"), "http://")
	return strings.TrimSuffix(addr, "/")
}

func serverHostPort(t *testing.T) (string, uint64, uint64) {
	t.Helper()
	host, portText, err := net.SplitHostPort(serverAddr())
	if err != nil {
		host, portText = serverAddr(), "8848"
	}
	port, err := strconv.ParseUint(portText, 10, 64)
	if err != nil || port == 0 {
		port = 8848
	}
	return host, port, port + uint64(constant.RpcPortOffset)
}

func serverConfigs(t *testing.T) []constant.ServerConfig {
	t.Helper()
	host, port, grpcPort := serverHostPort(t)
	return []constant.ServerConfig{{
		IpAddr:      host,
		Port:        port,
		GrpcPort:    grpcPort,
		Scheme:      "http",
		ContextPath: "/nacos",
	}}
}

func username() string { return envOr("NACOS_USERNAME", "nacos") }
func password() string { return envOr("NACOS_PASSWORD", "nacos") }

// newClientConfig deliberately disables the local snapshot: a stale snapshot
// would hide wire-level regressions by serving cached content without a server
// round-trip.
func newClientConfig(t *testing.T, namespace string) constant.ClientConfig {
	t.Helper()
	dir := t.TempDir()
	return constant.ClientConfig{
		TimeoutMs:           10000,
		NamespaceId:         namespace,
		Username:            username(),
		Password:            password(),
		NotLoadCacheAtStart: true,
		DisableUseSnapShot:  true,
		CacheDir:            dir,
		LogDir:              dir,
		LogLevel:            "error",
	}
}

func newConfigClient(t *testing.T, namespace string) config_client.IConfigClient {
	t.Helper()
	cfg := newClientConfig(t, namespace)
	client, err := clients.NewConfigClient(vo.NacosClientParam{
		ClientConfig:  &cfg,
		ServerConfigs: serverConfigs(t),
	})
	if err != nil {
		t.Fatalf("create config client (namespace=%q): %v", namespace, err)
	}
	t.Cleanup(client.CloseClient)
	return client
}

func newNamingClientWithOptions(
	t *testing.T,
	namespace string,
	configure func(*constant.ClientConfig),
) naming_client.INamingClient {
	t.Helper()
	cfg := newClientConfig(t, namespace)
	if configure != nil {
		configure(&cfg)
	}
	client, err := clients.NewNamingClient(vo.NacosClientParam{
		ClientConfig:  &cfg,
		ServerConfigs: serverConfigs(t),
	})
	if err != nil {
		t.Fatalf("create naming client (namespace=%q): %v", namespace, err)
	}
	t.Cleanup(client.CloseClient)
	return client
}

func newNamingClient(t *testing.T, namespace string) naming_client.INamingClient {
	return newNamingClientWithOptions(t, namespace, nil)
}

func waitFor(timeout time.Duration, cond func() bool) bool {
	deadline := time.Now().Add(timeout)
	for time.Now().Before(deadline) {
		if cond() {
			return true
		}
		// nacos-sdk-go rate-limits config queries to 5/s per cache key.
		// Polling faster than that makes the SDK return a local "limited"
		// error, which would be mistaken for a server regression.
		time.Sleep(250 * time.Millisecond)
	}
	return cond()
}

func md5Hex(s string) string {
	sum := md5.Sum([]byte(s))
	return hex.EncodeToString(sum[:])
}

func cleanupConfig(t *testing.T, client config_client.IConfigClient, dataID, group string) {
	t.Helper()
	t.Cleanup(func() {
		_, _ = client.DeleteConfig(vo.ConfigParam{DataId: dataID, Group: group})
	})
}

// ---- admin-only fixture helper ----
//
// Namespace creation has no public API in nacos-sdk-go, so this helper is the
// one intentional exception: it provisions fixtures through the server admin
// API. No SDK behavior is asserted through it.

func adminRequest(t *testing.T, method, path string, query, form url.Values, token string) (int, map[string]interface{}) {
	t.Helper()
	endpoint := "http://" + serverAddr() + path
	if len(query) > 0 {
		endpoint += "?" + query.Encode()
	}
	var body io.Reader
	if len(form) > 0 {
		body = strings.NewReader(form.Encode())
	}
	req, err := http.NewRequest(method, endpoint, body)
	if err != nil {
		t.Fatalf("build %s %s: %v", method, path, err)
	}
	if token != "" {
		req.Header.Set("accessToken", token)
	}
	if len(form) > 0 {
		req.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	}
	client := &http.Client{Timeout: 10 * time.Second, Transport: &http.Transport{Proxy: nil}}
	resp, err := client.Do(req)
	if err != nil {
		t.Fatalf("%s %s: %v", method, path, err)
	}
	defer resp.Body.Close()
	raw, err := io.ReadAll(resp.Body)
	if err != nil {
		t.Fatalf("read %s %s: %v", method, path, err)
	}
	out := map[string]interface{}{}
	if len(raw) > 0 {
		_ = json.Unmarshal(raw, &out)
	}
	return resp.StatusCode, out
}

func loginToken(t *testing.T) string {
	t.Helper()
	form := url.Values{"username": {username()}, "password": {password()}}
	_, body := adminRequest(t, http.MethodPost, "/nacos/v1/auth/users/login", nil, form, "")
	if token, ok := body["accessToken"].(string); ok && token != "" {
		return token
	}
	if data, ok := body["data"].(map[string]interface{}); ok {
		if token, ok := data["accessToken"].(string); ok {
			return token
		}
	}
	t.Fatalf("login response has no accessToken: %v", body)
	return ""
}

func createNamespace(t *testing.T, id string) {
	t.Helper()
	token := loginToken(t)
	form := url.Values{
		"customNamespaceId": {id},
		"namespaceName":     {id},
		"namespaceDesc":     {"nacos-go-sdk compatibility suite"},
	}
	_, body := adminRequest(t, http.MethodPost, "/nacos/v3/admin/core/namespace", nil, form, token)
	if code, _ := body["code"].(float64); code != 0 {
		t.Fatalf("create namespace %s: %v", id, body)
	}
	t.Cleanup(func() {
		token := loginToken(t)
		_, _ = adminRequest(t, http.MethodDelete, "/nacos/v3/admin/core/namespace",
			url.Values{"namespaceId": {id}}, nil, token)
	})
}
