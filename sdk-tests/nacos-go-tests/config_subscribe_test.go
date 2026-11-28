package nacos_go_tests

import (
	"testing"
	"time"

	"github.com/nacos-group/nacos-sdk-go/v2/vo"
)

type changeEvent struct {
	namespace string
	group     string
	dataID    string
	data      string
}

func TestConfigListenReceivesChange(t *testing.T) {
	client := newConfigClient(t, "")
	dataID := uniqueID("listen")
	cleanupConfig(t, client, dataID, defaultGroup)

	initial := "listen.version=1"
	if ok, err := client.PublishConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup, Content: initial}); err != nil || !ok {
		t.Fatalf("initial publish: ok=%v err=%v", ok, err)
	}
	events := make(chan changeEvent, 16)
	param := vo.ConfigParam{
		DataId: dataID,
		Group:  defaultGroup,
		OnChange: func(namespace, group, id, data string) {
			select {
			case events <- changeEvent{namespace, group, id, data}:
			default:
			}
		},
	}
	if err := client.ListenConfig(param); err != nil {
		t.Fatalf("ListenConfig: %v", err)
	}
	t.Cleanup(func() { _ = client.CancelListenConfig(param) })

	updated := "listen.version=2"
	if ok, err := client.PublishConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup, Content: updated}); err != nil || !ok {
		t.Fatalf("update publish: ok=%v err=%v", ok, err)
	}
	deadline := time.After(15 * time.Second)
	for {
		select {
		case ev := <-events:
			if ev.dataID == dataID && ev.group == defaultGroup && ev.data == updated {
				return
			}
		case <-deadline:
			t.Fatalf("no change callback carrying %q", updated)
		}
	}
}

func TestConfigListenCancelStopsCallbacks(t *testing.T) {
	client := newConfigClient(t, "")
	dataID := uniqueID("cancel-listen")
	cleanupConfig(t, client, dataID, defaultGroup)

	if ok, err := client.PublishConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup, Content: "v=1"}); err != nil || !ok {
		t.Fatalf("initial publish: ok=%v err=%v", ok, err)
	}
	events := make(chan changeEvent, 16)
	param := vo.ConfigParam{
		DataId: dataID,
		Group:  defaultGroup,
		OnChange: func(namespace, group, id, data string) {
			select {
			case events <- changeEvent{namespace, group, id, data}:
			default:
			}
		},
	}
	if err := client.ListenConfig(param); err != nil {
		t.Fatalf("ListenConfig: %v", err)
	}
	if ok, err := client.PublishConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup, Content: "v=2"}); err != nil || !ok {
		t.Fatalf("update publish: ok=%v err=%v", ok, err)
	}
	if !waitFor(15*time.Second, func() bool {
		select {
		case ev := <-events:
			return ev.dataID == dataID && ev.data == "v=2"
		default:
			return false
		}
	}) {
		t.Fatal("listener did not observe the pre-cancel update")
	}
	if err := client.CancelListenConfig(param); err != nil {
		t.Fatalf("CancelListenConfig: %v", err)
	}
	// Drain any duplicate deliveries queued before cancellation took effect.
	time.Sleep(500 * time.Millisecond)
	for len(events) > 0 {
		<-events
	}
	if ok, err := client.PublishConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup, Content: "v=3"}); err != nil || !ok {
		t.Fatalf("post-cancel publish: ok=%v err=%v", ok, err)
	}
	select {
	case ev := <-events:
		t.Fatalf("callback fired after CancelListenConfig: %+v", ev)
	case <-time.After(3 * time.Second):
	}
}

func TestConfigListenInNamedNamespace(t *testing.T) {
	ns := uniqueID("listen-ns")
	createNamespace(t, ns)
	client := newConfigClient(t, ns)
	dataID := uniqueID("listen-ns-cfg")
	cleanupConfig(t, client, dataID, defaultGroup)

	if ok, err := client.PublishConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup, Content: "ns=1"}); err != nil || !ok {
		t.Fatalf("publish in namespace: ok=%v err=%v", ok, err)
	}
	events := make(chan changeEvent, 16)
	param := vo.ConfigParam{
		DataId: dataID,
		Group:  defaultGroup,
		OnChange: func(namespace, group, id, data string) {
			select {
			case events <- changeEvent{namespace, group, id, data}:
			default:
			}
		},
	}
	if err := client.ListenConfig(param); err != nil {
		t.Fatalf("ListenConfig in namespace: %v", err)
	}
	t.Cleanup(func() { _ = client.CancelListenConfig(param) })
	if ok, err := client.PublishConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup, Content: "ns=2"}); err != nil || !ok {
		t.Fatalf("update in namespace: ok=%v err=%v", ok, err)
	}
	deadline := time.After(15 * time.Second)
	for {
		select {
		case ev := <-events:
			if ev.dataID == dataID && ev.data == "ns=2" {
				return
			}
		case <-deadline:
			t.Fatal("namespace-scoped listener never received the update")
		}
	}
}

func TestConfigNamespaceIsolation(t *testing.T) {
	nsA := uniqueID("ns-a")
	nsB := uniqueID("ns-b")
	createNamespace(t, nsA)
	createNamespace(t, nsB)
	clientA := newConfigClient(t, nsA)
	clientB := newConfigClient(t, nsB)
	dataID := uniqueID("shared")

	contentA := "tenant=A"
	contentB := "tenant=B"
	cleanupConfig(t, clientA, dataID, defaultGroup)
	cleanupConfig(t, clientB, dataID, defaultGroup)
	for _, c := range []struct {
		client interface {
			PublishConfig(vo.ConfigParam) (bool, error)
		}
		content string
	}{{clientA, contentA}, {clientB, contentB}} {
		if ok, err := c.client.PublishConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup, Content: c.content}); err != nil || !ok {
			t.Fatalf("publish %q: ok=%v err=%v", c.content, ok, err)
		}
	}
	if !waitFor(10*time.Second, func() bool {
		a, errA := clientA.GetConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup})
		b, errB := clientB.GetConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup})
		return errA == nil && errB == nil && a == contentA && b == contentB
	}) {
		a, errA := clientA.GetConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup})
		b, errB := clientB.GetConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup})
		t.Fatalf("namespace isolation broken: A=%q/%v B=%q/%v", a, errA, b, errB)
	}
}

func TestConfigEmptyNamespaceEqualsPublicTenant(t *testing.T) {
	defaultClient := newConfigClient(t, "")
	publicClient := newConfigClient(t, "public")
	dataID := uniqueID("public-tenant")
	cleanupConfig(t, defaultClient, dataID, defaultGroup)

	content := "tenant=public"
	if ok, err := defaultClient.PublishConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup, Content: content}); err != nil || !ok {
		t.Fatalf("publish with empty namespace: ok=%v err=%v", ok, err)
	}
	if !waitFor(10*time.Second, func() bool {
		got, err := publicClient.GetConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup})
		return err == nil && got == content
	}) {
		got, err := publicClient.GetConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup})
		t.Fatalf("empty namespace did not map to public: got=%q err=%v", got, err)
	}
}

func TestConfigGrayConnectionLabelAccepted(t *testing.T) {
	// The Go SDK sends nacos_config_gray_label in ConnectionSetupRequest.Labels.
	// Batata must accept the extra label and still serve the normal config path.
	t.Setenv("nacos_config_gray_label", "batata-go-contract")
	client := newConfigClient(t, "")
	dataID := uniqueID("gray-label")
	cleanupConfig(t, client, dataID, defaultGroup)

	content := "gray.label=accepted"
	if ok, err := client.PublishConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup, Content: content}); err != nil || !ok {
		t.Fatalf("publish with gray label: ok=%v err=%v", ok, err)
	}
	if !waitFor(10*time.Second, func() bool {
		got, err := client.GetConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup})
		return err == nil && got == content
	}) {
		got, err := client.GetConfig(vo.ConfigParam{DataId: dataID, Group: defaultGroup})
		t.Fatalf("gray-labelled connection failed: got=%q err=%v", got, err)
	}
}
