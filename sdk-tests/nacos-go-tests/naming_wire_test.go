package nacos_go_tests

import (
	"testing"
	"time"

	"github.com/nacos-group/nacos-sdk-go/v2/common/constant"
	"github.com/nacos-group/nacos-sdk-go/v2/model"
	"github.com/nacos-group/nacos-sdk-go/v2/vo"
)

func registerTestInstance(t *testing.T, client interface {
	RegisterInstance(vo.RegisterInstanceParam) (bool, error)
	DeregisterInstance(vo.DeregisterInstanceParam) (bool, error)
}, service, ip string, port uint64, cluster string, metadata map[string]string, weight float64, healthy bool) {
	t.Helper()
	param := vo.RegisterInstanceParam{
		Ip: ip, Port: port, ServiceName: service, GroupName: defaultGroup,
		ClusterName: cluster, Weight: weight, Enable: true, Healthy: healthy,
		Ephemeral: true, Metadata: metadata,
	}
	ok, err := client.RegisterInstance(param)
	if err != nil || !ok {
		t.Fatalf("register %s/%s:%d: ok=%v err=%v", service, ip, port, ok, err)
	}
	t.Cleanup(func() {
		_, _ = client.DeregisterInstance(vo.DeregisterInstanceParam{
			Ip: ip, Port: port, ServiceName: service, GroupName: defaultGroup, Cluster: cluster, Ephemeral: true,
		})
	})
}

func waitForInstance(t *testing.T, client interface {
	GetService(vo.GetServiceParam) (model.Service, error)
}, service string, match func(model.Instance) bool) model.Instance {
	t.Helper()
	var found model.Instance
	if !waitFor(15*time.Second, func() bool {
		svc, err := client.GetService(vo.GetServiceParam{ServiceName: service, GroupName: defaultGroup})
		if err != nil {
			return false
		}
		for _, ins := range svc.Hosts {
			if match(ins) {
				found = ins
				return true
			}
		}
		return false
	}) {
		svc, err := client.GetService(vo.GetServiceParam{ServiceName: service, GroupName: defaultGroup})
		t.Fatalf("instance of %s never became visible: service=%+v err=%v", service, svc, err)
	}
	return found
}

func TestNamingRegisterAndGetService(t *testing.T) {
	client := newNamingClient(t, "")
	service := uniqueID("svc")
	registerTestInstance(t, client, service, "127.0.0.1", 18081, "DEFAULT", nil, 10, true)

	ins := waitForInstance(t, client, service, func(i model.Instance) bool {
		return i.Ip == "127.0.0.1" && i.Port == 18081
	})
	if ins.Weight != 10 || !ins.Healthy || !ins.Enable {
		t.Fatalf("instance fields distorted on the wire: %+v", ins)
	}
}

func TestNamingMetadataAndClusterRoundTrip(t *testing.T) {
	client := newNamingClient(t, "")
	service := uniqueID("meta")
	registerTestInstance(t, client, service, "127.0.0.2", 18082, "go-cluster",
		map[string]string{"idc": "shanghai", "version": "v2"}, 1, true)

	ins := waitForInstance(t, client, service, func(i model.Instance) bool {
		return i.Ip == "127.0.0.2" && i.Metadata["idc"] == "shanghai"
	})
	if ins.ClusterName != "go-cluster" || ins.Metadata["version"] != "v2" {
		t.Fatalf("metadata/cluster lost: %+v", ins)
	}
}

func TestNamingSelectInstancesHealthyOnly(t *testing.T) {
	client := newNamingClient(t, "")
	service := uniqueID("select")
	registerTestInstance(t, client, service, "127.0.0.3", 18083, "DEFAULT", nil, 1, true)

	var found []model.Instance
	if !waitFor(15*time.Second, func() bool {
		got, err := client.SelectInstances(vo.SelectInstancesParam{
			ServiceName: service, GroupName: defaultGroup, HealthyOnly: true,
		})
		if err == nil && len(got) == 1 && got[0].Ip == "127.0.0.3" {
			found = got
			return true
		}
		return false
	}) {
		t.Fatalf("healthy select did not return the instance: %+v", found)
	}
}

func TestNamingDeregisterRemovesInstance(t *testing.T) {
	// The official Go SDK defaults to push-empty protection: a pushed service
	// with zero hosts is ignored unless UpdateCacheWhenEmpty is enabled. Enable
	// it here so this case verifies deregistration visibility through SDK APIs.
	client := newNamingClientWithOptions(t, "", func(c *constant.ClientConfig) {
		c.UpdateCacheWhenEmpty = true
	})
	service := uniqueID("deregister")
	registerTestInstance(t, client, service, "127.0.0.4", 18084, "DEFAULT", nil, 1, true)
	waitForInstance(t, client, service, func(i model.Instance) bool { return i.Ip == "127.0.0.4" })

	ok, err := client.DeregisterInstance(vo.DeregisterInstanceParam{
		Ip: "127.0.0.4", Port: 18084, ServiceName: service, GroupName: defaultGroup,
		Cluster: "DEFAULT", Ephemeral: true,
	})
	if err != nil || !ok {
		t.Fatalf("deregister: ok=%v err=%v", ok, err)
	}
	// Observe with a fresh client: the registering client's local cache may
	// hold a pre-deregistration snapshot. A new client has no snapshot and
	// therefore asks the server through the SDK on the first GetService.
	verifier := newNamingClientWithOptions(t, "", func(c *constant.ClientConfig) {
		c.UpdateCacheWhenEmpty = true
	})
	if !waitFor(15*time.Second, func() bool {
		svc, err := verifier.GetService(vo.GetServiceParam{ServiceName: service, GroupName: defaultGroup})
		return err == nil && len(svc.Hosts) == 0
	}) {
		svc, err := verifier.GetService(vo.GetServiceParam{ServiceName: service, GroupName: defaultGroup})
		t.Fatalf("instance survived deregistration: %+v err=%v", svc.Hosts, err)
	}
}

func TestNamingBatchRegister(t *testing.T) {
	client := newNamingClient(t, "")
	service := uniqueID("batch")
	param := vo.BatchRegisterInstanceParam{
		ServiceName: service,
		GroupName:   defaultGroup,
		Instances: []vo.RegisterInstanceParam{
			{Ip: "127.0.0.5", Port: 18085, ServiceName: service, GroupName: defaultGroup, Weight: 1, Enable: true, Healthy: true, Ephemeral: true, ClusterName: "DEFAULT"},
			{Ip: "127.0.0.6", Port: 18086, ServiceName: service, GroupName: defaultGroup, Weight: 2, Enable: true, Healthy: true, Ephemeral: true, ClusterName: "DEFAULT"},
		},
	}
	ok, err := client.BatchRegisterInstance(param)
	if err != nil || !ok {
		t.Fatalf("batch register: ok=%v err=%v", ok, err)
	}
	for _, ip := range []string{"127.0.0.5", "127.0.0.6"} {
		target := ip
		t.Cleanup(func() {
			_, _ = client.DeregisterInstance(vo.DeregisterInstanceParam{
				Ip: target, Port: map[string]uint64{"127.0.0.5": 18085, "127.0.0.6": 18086}[target],
				ServiceName: service, GroupName: defaultGroup, Cluster: "DEFAULT", Ephemeral: true,
			})
		})
	}
	if !waitFor(15*time.Second, func() bool {
		svc, err := client.GetService(vo.GetServiceParam{ServiceName: service, GroupName: defaultGroup})
		return err == nil && len(svc.Hosts) == 2
	}) {
		svc, err := client.GetService(vo.GetServiceParam{ServiceName: service, GroupName: defaultGroup})
		t.Fatalf("batch register result: %+v err=%v", svc.Hosts, err)
	}
}

func TestNamingUpdateInstanceWeight(t *testing.T) {
	client := newNamingClient(t, "")
	service := uniqueID("update")
	registerTestInstance(t, client, service, "127.0.0.7", 18087, "DEFAULT", nil, 1, true)
	waitForInstance(t, client, service, func(i model.Instance) bool { return i.Ip == "127.0.0.7" })

	ok, err := client.UpdateInstance(vo.UpdateInstanceParam{
		Ip: "127.0.0.7", Port: 18087, ServiceName: service, GroupName: defaultGroup,
		ClusterName: "DEFAULT", Weight: 7, Enable: true, Healthy: true, Ephemeral: true,
	})
	if err != nil || !ok {
		t.Fatalf("update instance: ok=%v err=%v", ok, err)
	}
	// A fresh SDK client avoids depending on the registering client's push
	// cache; its first GetService is a real server query through the SDK.
	verifier := newNamingClient(t, "")
	if !waitFor(15*time.Second, func() bool {
		svc, err := verifier.GetService(vo.GetServiceParam{ServiceName: service, GroupName: defaultGroup})
		if err != nil {
			return false
		}
		for _, ins := range svc.Hosts {
			if ins.Ip == "127.0.0.7" && ins.Weight == 7 {
				return true
			}
		}
		return false
	}) {
		svc, err := verifier.GetService(vo.GetServiceParam{ServiceName: service, GroupName: defaultGroup})
		t.Fatalf("updated weight was not reflected: %+v err=%v", svc.Hosts, err)
	}
}

func TestNamingSubscribeReceivesPush(t *testing.T) {
	client := newNamingClient(t, "")
	service := uniqueID("subscribe")
	registerTestInstance(t, client, service, "127.0.0.8", 18088, "DEFAULT", nil, 1, true)
	// Register first: the Go SDK delivers the current instance list in the
	// first callback, which removes the subscribe-before-register race.
	events := make(chan []model.Instance, 16)
	param := &vo.SubscribeParam{
		ServiceName: service,
		GroupName:   defaultGroup,
		SubscribeCallback: func(services []model.Instance, err error) {
			if err != nil {
				return
			}
			select {
			case events <- services:
			default:
			}
		},
	}
	if err := client.Subscribe(param); err != nil {
		t.Fatalf("Subscribe: %v", err)
	}
	t.Cleanup(func() { _ = client.Unsubscribe(param) })

	deadline := time.After(15 * time.Second)
	for {
		select {
		case services := <-events:
			for _, ins := range services {
				if ins.Ip == "127.0.0.8" && ins.Port == 18088 {
					return
				}
			}
		case <-deadline:
			t.Fatal("subscribe callback never delivered the registered instance")
		}
	}
}

func TestNamingGetAllServicesInfo(t *testing.T) {
	client := newNamingClient(t, "")
	service := uniqueID("list")
	registerTestInstance(t, client, service, "127.0.0.9", 18089, "DEFAULT", nil, 1, true)
	waitForInstance(t, client, service, func(i model.Instance) bool { return i.Ip == "127.0.0.9" })

	if !waitFor(15*time.Second, func() bool {
		page, err := client.GetAllServicesInfo(vo.GetAllServiceInfoParam{
			GroupName: defaultGroup, PageNo: 1, PageSize: 100,
		})
		if err != nil {
			return false
		}
		for _, name := range page.Doms {
			if name == service {
				return true
			}
		}
		return false
	}) {
		page, err := client.GetAllServicesInfo(vo.GetAllServiceInfoParam{GroupName: defaultGroup, PageNo: 1, PageSize: 100})
		t.Fatalf("service list missing %s: %+v err=%v", service, page, err)
	}
}

func TestNamingNamespaceIsolation(t *testing.T) {
	nsA := uniqueID("naming-ns-a")
	nsB := uniqueID("naming-ns-b")
	createNamespace(t, nsA)
	createNamespace(t, nsB)
	clientA := newNamingClient(t, nsA)
	clientB := newNamingClient(t, nsB)
	service := uniqueID("shared-svc")
	registerTestInstance(t, clientA, service, "10.0.0.1", 19001, "DEFAULT", nil, 1, true)
	registerTestInstance(t, clientB, service, "10.0.0.2", 19002, "DEFAULT", nil, 1, true)

	waitForInstance(t, clientA, service, func(i model.Instance) bool { return i.Ip == "10.0.0.1" })
	waitForInstance(t, clientB, service, func(i model.Instance) bool { return i.Ip == "10.0.0.2" })
	svcA, err := clientA.GetService(vo.GetServiceParam{ServiceName: service, GroupName: defaultGroup})
	if err != nil || len(svcA.Hosts) != 1 || svcA.Hosts[0].Ip != "10.0.0.1" {
		t.Fatalf("tenant A view: %+v err=%v", svcA.Hosts, err)
	}
	svcB, err := clientB.GetService(vo.GetServiceParam{ServiceName: service, GroupName: defaultGroup})
	if err != nil || len(svcB.Hosts) != 1 || svcB.Hosts[0].Ip != "10.0.0.2" {
		t.Fatalf("tenant B view: %+v err=%v", svcB.Hosts, err)
	}
}

func TestNamingEmptyNamespaceMapsToPublic(t *testing.T) {
	defaultClient := newNamingClient(t, "")
	publicClient := newNamingClient(t, "public")
	service := uniqueID("public-svc")
	registerTestInstance(t, defaultClient, service, "10.1.0.1", 19101, "DEFAULT", nil, 1, true)
	waitForInstance(t, publicClient, service, func(i model.Instance) bool { return i.Ip == "10.1.0.1" })
}

func TestNamingEmptyServiceNameRejected(t *testing.T) {
	client := newNamingClient(t, "")
	ok, err := client.RegisterInstance(vo.RegisterInstanceParam{
		Ip: "127.0.0.1", Port: 19999, Weight: 1, Enable: true, Healthy: true, Ephemeral: true,
	})
	if err == nil || ok {
		t.Fatalf("empty service name should be rejected locally: ok=%v err=%v", ok, err)
	}
}
