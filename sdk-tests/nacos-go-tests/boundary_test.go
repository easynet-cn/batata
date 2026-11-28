package nacos_go_tests

import (
	"strings"
	"testing"
	"time"

	"github.com/nacos-group/nacos-sdk-go/v2/vo"
)

func TestConfigDefaultGroupWhenOmitted(t *testing.T) {
	client := newConfigClient(t, "")
	dataID := uniqueID("default-group")
	content := "group=default"
	cleanupConfig(t, client, dataID, defaultGroup)

	ok, err := client.PublishConfig(vo.ConfigParam{DataId: dataID, Content: content})
	if err != nil || !ok {
		t.Fatalf("publish without group: ok=%v err=%v", ok, err)
	}
	if !waitFor(10*time.Second, func() bool {
		got, err := client.GetConfig(vo.ConfigParam{DataId: dataID})
		return err == nil && got == content
	}) {
		t.Fatal("SDK default group did not match the server's DEFAULT_GROUP")
	}
}

func TestConfigLargeContentRoundTrip(t *testing.T) {
	client := newConfigClient(t, "")
	dataID := uniqueID("large")
	content := strings.Repeat("line-with-中文-and-symbols=!@#$%^&*()\n", 1600)
	cleanupConfig(t, client, dataID, defaultGroup)

	ok, err := client.PublishConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup, Content: content})
	if err != nil || !ok {
		t.Fatalf("publish large content: ok=%v err=%v", ok, err)
	}
	if !waitFor(15*time.Second, func() bool {
		got, err := client.GetConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup})
		return err == nil && got == content
	}) {
		got, err := client.GetConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup})
		t.Fatalf("large content round trip failed: gotLen=%d wantLen=%d err=%v", len(got), len(content), err)
	}
}

func TestNamingDefaultGroupWhenOmitted(t *testing.T) {
	client := newNamingClient(t, "")
	service := uniqueID("default-group-svc")
	ok, err := client.RegisterInstance(vo.RegisterInstanceParam{
		Ip: "127.0.0.11", Port: 19201, ServiceName: service,
		Weight: 1, Enable: true, Healthy: true, Ephemeral: true,
	})
	if err != nil || !ok {
		t.Fatalf("register without group: ok=%v err=%v", ok, err)
	}
	t.Cleanup(func() {
		_, _ = client.DeregisterInstance(vo.DeregisterInstanceParam{
			Ip: "127.0.0.11", Port: 19201, ServiceName: service, Ephemeral: true,
		})
	})
	if !waitFor(15*time.Second, func() bool {
		svc, err := client.GetService(vo.GetServiceParam{ServiceName: service, GroupName: defaultGroup})
		if err != nil {
			return false
		}
		for _, ins := range svc.Hosts {
			if ins.Ip == "127.0.0.11" && ins.Port == 19201 {
				return true
			}
		}
		return false
	}) {
		svc, err := client.GetService(vo.GetServiceParam{ServiceName: service, GroupName: defaultGroup})
		t.Fatalf("naming default group mismatch: %+v err=%v", svc.Hosts, err)
	}
}
