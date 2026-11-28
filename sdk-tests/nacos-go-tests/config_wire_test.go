package nacos_go_tests

import (
	"testing"
	"time"

	"github.com/nacos-group/nacos-sdk-go/v2/vo"
)

func TestConfigPublishGetRoundTrip(t *testing.T) {
	client := newConfigClient(t, "")
	dataID := uniqueID("basic")
	content := "app.name=batata\ngo.sdk=official\n"
	cleanupConfig(t, client, dataID, defaultGroup)

	ok, err := client.PublishConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup, Content: content})
	if err != nil || !ok {
		t.Fatalf("publish: ok=%v err=%v", ok, err)
	}
	if !waitFor(10*time.Second, func() bool {
		got, err := client.GetConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup})
		return err == nil && got == content
	}) {
		got, err := client.GetConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup})
		t.Fatalf("get after publish: got=%q err=%v", got, err)
	}
}

func TestConfigUnicodeAndMultilineRoundTrip(t *testing.T) {
	client := newConfigClient(t, "")
	dataID := uniqueID("unicode")
	content := "# 配置中心\nname: 中文键\nvalue: \"引号 / 反斜杠 \\\\ 与\t制表\"\nlist:\n  - alpha\n  - beta\n"
	cleanupConfig(t, client, dataID, defaultGroup)

	ok, err := client.PublishConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup, Content: content})
	if err != nil || !ok {
		t.Fatalf("publish unicode: ok=%v err=%v", ok, err)
	}
	if !waitFor(10*time.Second, func() bool {
		got, err := client.GetConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup})
		return err == nil && got == content
	}) {
		got, err := client.GetConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup})
		t.Fatalf("unicode round trip: got=%q err=%v", got, err)
	}
}

func TestConfigDeleteRemovesContent(t *testing.T) {
	client := newConfigClient(t, "")
	dataID := uniqueID("delete")
	param := vo.ConfigParam{DataId: dataID, Group: defaultGroup, Content: "before-delete=1"}
	ok, err := client.PublishConfig(param)
	if err != nil || !ok {
		t.Fatalf("publish: ok=%v err=%v", ok, err)
	}
	if !waitFor(10*time.Second, func() bool {
		got, _ := client.GetConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup})
		return got == param.Content
	}) {
		t.Fatal("config was not visible before delete")
	}

	deleted, err := client.DeleteConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup})
	if err != nil || !deleted {
		t.Fatalf("delete: ok=%v err=%v", deleted, err)
	}
	if !waitFor(10*time.Second, func() bool {
		got, _ := client.GetConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup})
		return got == ""
	}) {
		got, err := client.GetConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup})
		t.Fatalf("content survived delete: got=%q err=%v", got, err)
	}
}

func TestConfigCasMd5Accepted(t *testing.T) {
	client := newConfigClient(t, "")
	dataID := uniqueID("cas-ok")
	cleanupConfig(t, client, dataID, defaultGroup)

	first := "cas.version=1"
	if ok, err := client.PublishConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup, Content: first}); err != nil || !ok {
		t.Fatalf("initial publish: ok=%v err=%v", ok, err)
	}
	var md5 string
	if !waitFor(10*time.Second, func() bool {
		got, err := client.GetConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup})
		if err == nil && got == first {
			md5 = md5Hex(got)
			return true
		}
		return false
	}) {
		t.Fatal("initial content never became visible")
	}
	second := "cas.version=2"
	ok, err := client.PublishConfig(vo.ConfigParam{
		DataId: dataID, Group: defaultGroup, Content: second, CasMd5: md5,
	})
	if err != nil || !ok {
		t.Fatalf("CAS publish with current md5: ok=%v err=%v", ok, err)
	}
	if !waitFor(10*time.Second, func() bool {
		got, _ := client.GetConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup})
		return got == second
	}) {
		t.Fatal("CAS update was not applied")
	}
}

func TestConfigCasMd5Rejected(t *testing.T) {
	client := newConfigClient(t, "")
	dataID := uniqueID("cas-bad")
	cleanupConfig(t, client, dataID, defaultGroup)

	if ok, err := client.PublishConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup, Content: "stable=1"}); err != nil || !ok {
		t.Fatalf("initial publish: ok=%v err=%v", ok, err)
	}
	ok, err := client.PublishConfig(vo.ConfigParam{
		DataId: dataID, Group: defaultGroup, Content: "must-not-land=1", CasMd5: md5Hex("stale"),
	})
	if err == nil && ok {
		t.Fatal("stale casMd5 was accepted")
	}
	got, getErr := client.GetConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup})
	if getErr != nil || got != "stable=1" {
		t.Fatalf("rejected CAS changed content: got=%q err=%v", got, getErr)
	}
}

func TestConfigPublishWithTypeAndAppName(t *testing.T) {
	client := newConfigClient(t, "")
	dataID := uniqueID("yaml") + ".yaml"
	cleanupConfig(t, client, dataID, defaultGroup)

	content := "server:\n  port: 8080\n"
	ok, err := client.PublishConfig(vo.ConfigParam{
		DataId: dataID, Group: defaultGroup, Content: content, Type: "yaml", AppName: "batata-go-tests",
	})
	if err != nil || !ok {
		t.Fatalf("publish with type/appName: ok=%v err=%v", ok, err)
	}
	if !waitFor(10*time.Second, func() bool {
		got, err := client.GetConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup})
		return err == nil && got == content
	}) {
		t.Fatal("typed config did not round trip")
	}
}

func TestConfigPublishWithConfigTags(t *testing.T) {
	client := newConfigClient(t, "")
	dataID := uniqueID("tags")
	cleanupConfig(t, client, dataID, defaultGroup)

	content := "config.tags=alpha,beta"
	ok, err := client.PublishConfig(vo.ConfigParam{
		DataId: dataID, Group: defaultGroup, Content: content, ConfigTags: "alpha,beta",
	})
	if err != nil || !ok {
		t.Fatalf("publish with configTags: ok=%v err=%v", ok, err)
	}
	if !waitFor(10*time.Second, func() bool {
		got, err := client.GetConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup})
		return err == nil && got == content
	}) {
		t.Fatal("config with configTags did not round trip")
	}
}

func TestConfigPublishWithGrayTag(t *testing.T) {
	client := newConfigClient(t, "")
	dataID := uniqueID("tag")
	cleanupConfig(t, client, dataID, defaultGroup)

	base := "release=base"
	if ok, err := client.PublishConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup, Content: base}); err != nil || !ok {
		t.Fatalf("base publish: ok=%v err=%v", ok, err)
	}
	gray := "release=gray"
	ok, err := client.PublishConfig(vo.ConfigParam{
		DataId: dataID, Group: defaultGroup, Content: gray, Tag: "go-canary",
	})
	if err != nil || !ok {
		t.Fatalf("publish with gray tag: ok=%v err=%v", ok, err)
	}
	if !waitFor(10*time.Second, func() bool {
		got, err := client.GetConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup})
		return err == nil && got == base
	}) {
		got, err := client.GetConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup})
		t.Fatalf("unexpected base content after gray publish: got=%q err=%v", got, err)
	}
}

func TestConfigPublishWithBetaIps(t *testing.T) {
	client := newConfigClient(t, "")
	dataID := uniqueID("beta")
	cleanupConfig(t, client, dataID, defaultGroup)

	base := "beta.base=1"
	if ok, err := client.PublishConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup, Content: base}); err != nil || !ok {
		t.Fatalf("base publish: ok=%v err=%v", ok, err)
	}
	if !waitFor(10*time.Second, func() bool {
		got, err := client.GetConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup})
		return err == nil && got == base
	}) {
		got, err := client.GetConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup})
		t.Fatalf("base publish not visible: got=%q err=%v", got, err)
	}

	// The SDK connection originates from 127.0.0.1 in this harness. Targeting
	// that peer with betaIps makes the beta content observable through the SDK
	// itself, so the assertion validates SDK-visible routing instead of a raw
	// admin endpoint.
	beta := "beta.gray=2"
	ok, err := client.PublishConfig(vo.ConfigParam{
		DataId: dataID, Group: defaultGroup, Content: beta, BetaIps: "127.0.0.1",
	})
	if err != nil || !ok {
		t.Fatalf("publish with betaIps: ok=%v err=%v", ok, err)
	}
	if !waitFor(10*time.Second, func() bool {
		got, err := client.GetConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup})
		return err == nil && got == beta
	}) {
		got, err := client.GetConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup})
		t.Fatalf("SDK did not receive beta content: got=%q err=%v", got, err)
	}
}

func TestConfigEmptyDataIdRejectedBySDK(t *testing.T) {
	client := newConfigClient(t, "")
	if ok, err := client.PublishConfig(vo.ConfigParam{Group: defaultGroup, Content: "irrelevant"}); err == nil || ok {
		t.Fatalf("empty dataId should be rejected locally: ok=%v err=%v", ok, err)
	}
	if _, err := client.GetConfig(vo.ConfigParam{Group: defaultGroup}); err == nil {
		t.Fatal("empty dataId GetConfig should be rejected locally")
	}
}

func TestConfigSearchConfigPaging(t *testing.T) {
	client := newConfigClient(t, "")
	dataID := uniqueID("search")
	cleanupConfig(t, client, dataID, defaultGroup)
	if ok, err := client.PublishConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup, Content: "search=me"}); err != nil || !ok {
		t.Fatalf("publish: ok=%v err=%v", ok, err)
	}
	if !waitFor(10*time.Second, func() bool {
		page, err := client.SearchConfig(vo.SearchConfigParam{Search: "accurate", DataId: dataID, Group: defaultGroup, PageNo: 1, PageSize: 10})
		return err == nil && page != nil && page.TotalCount >= 1
	}) {
		page, err := client.SearchConfig(vo.SearchConfigParam{Search: "accurate", DataId: dataID, Group: defaultGroup, PageNo: 1, PageSize: 10})
		t.Fatalf("search config: page=%+v err=%v", page, err)
	}
}
