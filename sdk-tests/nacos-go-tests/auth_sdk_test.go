package nacos_go_tests

import (
	"testing"
	"time"

	"github.com/nacos-group/nacos-sdk-go/v2/clients"
	"github.com/nacos-group/nacos-sdk-go/v2/vo"
)

// Auth coverage intentionally exercises only the official SDK's public API.
// Raw HTTP login / accessToken-header assertions were removed: they bypass the
// SDK credential chain and therefore test server endpoints, not the SDK.

func TestAuthWrongPasswordSDKCannotReadConfig(t *testing.T) {
	cfg := newClientConfig(t, "")
	cfg.Password = "definitely-wrong-password"
	cfg.TimeoutMs = 2000
	client, err := clients.NewConfigClient(vo.NacosClientParam{
		ClientConfig: &cfg, ServerConfigs: serverConfigs(t),
	})
	if err != nil {
		t.Fatalf("client construction should not require a successful login: %v", err)
	}
	t.Cleanup(client.CloseClient)

	got, err := client.GetConfig(vo.ConfigParam{DataId: uniqueID("wrong-password"), Group: defaultGroup})
	if err == nil {
		t.Fatalf("SDK read succeeded with a wrong password: value=%q", got)
	}
}

func TestAuthSDKCredentialsDriveRealOperations(t *testing.T) {
	client := newConfigClient(t, "")
	dataID := uniqueID("auth-ok")
	cleanupConfig(t, client, dataID, defaultGroup)
	content := "authenticated=true"

	ok, err := client.PublishConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup, Content: content})
	if err != nil || !ok {
		t.Fatalf("authenticated publish: ok=%v err=%v", ok, err)
	}
	if !waitFor(10*time.Second, func() bool {
		got, err := client.GetConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup})
		return err == nil && got == content
	}) {
		t.Fatal("authenticated SDK operations did not round trip")
	}
}
