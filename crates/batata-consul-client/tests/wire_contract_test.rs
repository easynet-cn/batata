//! Golden wire-contract tests for acronym field names.
//!
//! Every assertion here is derived from the live HashiCorp Consul `api/`
//! package (baseline `98fd79d318`). Go's `encoding/json` uses the Go field
//! name verbatim when a field has no explicit `json:"..."` name - that is
//! why upstream emits `SDS`, `TLSMinVersion`, `WAN` etc. rather than the
//! PascalCase spellings serde's `rename_all` would produce.
//!
//! Each test asserts both directions so a mutant that drops either `rename`
//! (serialization) or `alias` (legacy deserialization) dies on a real value
//! assertion, not just a compile error.

use batata_consul_client::config_entry_types::*;
use batata_consul_client::discovery_chain::DiscoveryTarget;
use batata_consul_client::*;
use serde_json::json;

fn keys(value: &serde_json::Value) -> Vec<&str> {
    value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect()
}

#[test]
fn gateway_tls_serializes_upstream_acronym_keys() {
    let cfg = GatewayTLSConfig {
        enabled: true,
        sds: Some(GatewayTLSSDSConfig {
            cluster_name: "static-cluster".into(),
            cert_resource: "cert".into(),
        }),
        tls_min_version: "TLSv1_2".into(),
        tls_max_version: "TLSv1_3".into(),
        cipher_suites: vec!["TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256".into()],
    };
    let obj = serde_json::to_value(&cfg).unwrap();
    let k = keys(&obj);
    assert!(k.contains(&"SDS"), "missing SDS: {k:?}");
    assert!(k.contains(&"TLSMinVersion"), "missing TLSMinVersion: {k:?}");
    assert!(k.contains(&"TLSMaxVersion"), "missing TLSMaxVersion: {k:?}");
    assert!(!k.contains(&"Sds"), "wrong local spelling Sds: {k:?}");
    assert!(!k.contains(&"TlsMinVersion"), "wrong TlsMinVersion: {k:?}");
    assert_eq!(obj["SDS"]["ClusterName"], "static-cluster");
    assert_eq!(obj["TLSMinVersion"], "TLSv1_2");

    // Upstream key (rename) and legacy local spelling (alias) must both work.
    let upstream: GatewayTLSConfig = serde_json::from_value(json!({
        "SDS": {"ClusterName":"upstream","CertResource":"cert"},
        "TLSMinVersion": "TLSv1_3"
    }))
    .unwrap();
    assert_eq!(upstream.sds.unwrap().cluster_name, "upstream");
    assert_eq!(upstream.tls_min_version, "TLSv1_3");

    let legacy: GatewayTLSConfig = serde_json::from_value(json!({
        "Sds": {"ClusterName":"legacy","CertResource":"cert"},
        "TlsMinVersion": "TLSv1_2"
    }))
    .unwrap();
    assert_eq!(legacy.sds.unwrap().cluster_name, "legacy");
    assert_eq!(legacy.tls_min_version, "TLSv1_2");
}

#[test]
fn ingress_gateway_tls_and_listener_service_keys_match_upstream() {
    let entry = IngressGatewayConfigEntry {
        tls: GatewayTLSConfig {
            tls_min_version: "TLSv1_2".into(),
            ..Default::default()
        },
        listeners: vec![IngressListener {
            tls: Some(GatewayTLSConfig {
                tls_max_version: "TLSv1_3".into(),
                ..Default::default()
            }),
            ..Default::default()
        }],
        ..Default::default()
    };
    let obj = serde_json::to_value(&entry).unwrap();
    assert_eq!(obj["TLS"]["TLSMinVersion"], "TLSv1_2");
    assert_eq!(obj["Listeners"][0]["TLS"]["TLSMaxVersion"], "TLSv1_3");
    assert!(obj.get("Tls").is_none());
    assert!(obj["Listeners"][0].get("Tls").is_none());

    let legacy: IngressGatewayConfigEntry = serde_json::from_value(json!({
        "Name":"gateway",
        "Tls": {"TlsMinVersion":"TLSv1_2"},
        "Listeners": [{"Port":8080,"Protocol":"http","Services":[],"Tls":{"TlsMaxVersion":"TLSv1_3"}}]
    }))
    .unwrap();
    assert_eq!(legacy.tls.tls_min_version, "TLSv1_2");
    assert_eq!(
        legacy.listeners[0].tls.as_ref().unwrap().tls_max_version,
        "TLSv1_3"
    );
}

#[test]
fn ingress_service_tls_and_gateway_service_sds_keys_match_upstream() {
    let svc = IngressService {
        tls: Some(GatewayServiceTLSConfig {
            sds: Some(GatewayTLSSDSConfig {
                cluster_name: "svc-cluster".into(),
                ..Default::default()
            }),
        }),
        ..Default::default()
    };
    let obj = serde_json::to_value(&svc).unwrap();
    assert_eq!(obj["TLS"]["SDS"]["ClusterName"], "svc-cluster");

    let legacy: IngressService = serde_json::from_value(json!({
        "Name":"web",
        "Tls": {"Sds":{"ClusterName":"legacy","CertResource":"cert"}}
    }))
    .unwrap();
    assert_eq!(legacy.tls.unwrap().sds.unwrap().cluster_name, "legacy");
}

#[test]
fn linked_service_sni_key_matches_upstream() {
    let svc = LinkedService {
        sni: "api.example.com".into(),
        ..Default::default()
    };
    let obj = serde_json::to_value(&svc).unwrap();
    assert_eq!(obj["SNI"], "api.example.com");
    assert!(obj.get("Sni").is_none());

    let legacy: LinkedService =
        serde_json::from_value(json!({"Name":"svc","Sni":"legacy.example"})).unwrap();
    assert_eq!(legacy.sni, "legacy.example");
}

#[test]
fn mesh_config_entry_acronym_keys_match_upstream() {
    let entry = MeshConfigEntry {
        allow_enabling_permissive_mutual_tls: Some(true),
        tls: MeshTLSConfig {
            incoming: Some(MeshDirectionalTLSConfig {
                tls_min_version: "TLSv1_2".into(),
                tls_max_version: "TLSv1_3".into(),
                ..Default::default()
            }),
            ..Default::default()
        },
        http: MeshHTTPConfig {
            sanitize_x_forwarded_client_cert: true,
        },
        ..Default::default()
    };
    let obj = serde_json::to_value(&entry).unwrap();
    assert_eq!(obj["AllowEnablingPermissiveMutualTLS"], true);
    assert_eq!(obj["TLS"]["Incoming"]["TLSMinVersion"], "TLSv1_2");
    assert_eq!(obj["TLS"]["Incoming"]["TLSMaxVersion"], "TLSv1_3");
    assert_eq!(obj["HTTP"]["SanitizeXForwardedClientCert"], true);
    for wrong in [
        "AllowEnablingPermissiveMutualTls",
        "Tls",
        "Http",
        "TlsMinVersion",
        "TlsMaxVersion",
    ] {
        assert!(
            !format!("{obj}").contains(&format!("\"{wrong}\"")),
            "wrong local spelling {wrong} leaked into wire JSON"
        );
    }

    let legacy: MeshConfigEntry = serde_json::from_value(json!({
        "Name":"mesh",
        "AllowEnablingPermissiveMutualTls": true,
        "Tls": {"Incoming":{"TlsMinVersion":"TLSv1_2","TlsMaxVersion":"TLSv1_3"}},
        "Http": {"SanitizeXForwardedClientCert": true}
    }))
    .unwrap();
    assert_eq!(legacy.allow_enabling_permissive_mutual_tls, Some(true));
    let incoming = legacy.tls.incoming.unwrap();
    assert_eq!(incoming.tls_min_version, "TLSv1_2");
    assert_eq!(incoming.tls_max_version, "TLSv1_3");
    assert!(legacy.http.sanitize_x_forwarded_client_cert);
}

#[test]
fn jwt_provider_and_service_intentions_acronym_keys_match_upstream() {
    let jwt = JWTProviderConfigEntry {
        json_web_key_set: Some(json!({"keys": []})),
        ..Default::default()
    };
    let obj = serde_json::to_value(&jwt).unwrap();
    assert!(obj.get("JSONWebKeySet").is_some());
    assert!(obj.get("JsonWebKeySet").is_none());
    let legacy: JWTProviderConfigEntry =
        serde_json::from_value(json!({"Name":"jwt","JsonWebKeySet":{"keys":[]}})).unwrap();
    assert!(legacy.json_web_key_set.is_some());

    let intentions = ServiceIntentionsConfigEntry {
        jwt: Some(json!({"Require": {}})),
        ..Default::default()
    };
    let obj = serde_json::to_value(&intentions).unwrap();
    assert!(obj.get("JWT").is_some());
    assert!(obj.get("Jwt").is_none());
    let legacy: ServiceIntentionsConfigEntry =
        serde_json::from_value(json!({"Name":"web","Jwt":{"Require":{}}})).unwrap();
    assert!(legacy.jwt.is_some());
}

#[test]
fn service_defaults_external_sni_key_matches_upstream() {
    let entry = ServiceDefaultsConfigEntry {
        external_sni: Some("api.example.com".into()),
        ..Default::default()
    };
    let obj = serde_json::to_value(&entry).unwrap();
    assert_eq!(obj["ExternalSNI"], "api.example.com");
    assert!(obj.get("ExternalSni").is_none());
    let legacy: ServiceDefaultsConfigEntry =
        serde_json::from_value(json!({"Name":"web","ExternalSni":"legacy.example"})).unwrap();
    assert_eq!(legacy.external_sni.as_deref(), Some("legacy.example"));
}

#[test]
fn keyring_response_wan_key_matches_upstream() {
    let resp = KeyringResponse {
        wan: true,
        datacenter: "dc1".into(),
        segment: String::new(),
        keys: Default::default(),
        primary_keys: Default::default(),
        num_nodes: 0,
    };
    let obj = serde_json::to_value(&resp).unwrap();
    assert_eq!(obj["WAN"], true);
    assert!(obj.get("Wan").is_none());
    let legacy: KeyringResponse =
        serde_json::from_value(json!({"Datacenter":"dc1","Wan":true})).unwrap();
    assert!(legacy.wan);
}

#[test]
fn intention_namespace_keys_match_upstream_go_field_names() {
    let intention = Intention {
        source_namespace: "ns1".into(),
        destination_namespace: "ns2".into(),
        ..Default::default()
    };
    let obj = serde_json::to_value(&intention).unwrap();
    assert_eq!(obj["SourceNS"], "ns1");
    assert_eq!(obj["DestinationNS"], "ns2");
    assert!(obj.get("SourceNamespace").is_none());
    assert!(obj.get("DestinationNamespace").is_none());
    let legacy: Intention = serde_json::from_value(json!({
        "SourceNamespace":"legacy-ns1","DestinationNamespace":"legacy-ns2"
    }))
    .unwrap();
    assert_eq!(legacy.source_namespace, "legacy-ns1");
    assert_eq!(legacy.destination_namespace, "legacy-ns2");
}

#[test]
fn discovery_target_sni_key_matches_upstream() {
    let target = DiscoveryTarget {
        sni: Some("api.example.com".into()),
        ..Default::default()
    };
    let obj = serde_json::to_value(&target).unwrap();
    assert_eq!(obj["SNI"], "api.example.com");
    assert!(obj.get("Sni").is_none());
    let legacy: DiscoveryTarget =
        serde_json::from_value(json!({"ID":"t1","Service":"web","Sni":"legacy.example"})).unwrap();
    assert_eq!(legacy.sni.as_deref(), Some("legacy.example"));
}

// ---------------------------------------------------------------------------
// P1: high-frequency endpoint DTOs - golden wire payloads adapted from the
// upstream Consul `api/` response shapes. Each test asserts populated values
// so dropping a field or its serde rename fails a real assertion.
// ---------------------------------------------------------------------------

#[test]
fn agent_service_deserializes_full_upstream_payload() {
    let svc: AgentService = serde_json::from_value(json!({
        "Kind": "typical",
        "ID": "web-1",
        "Service": "web",
        "Tags": ["v1"],
        "Meta": {"env": "prod"},
        "Port": 8080,
        "Ports": {"http": 8080},
        "Address": "10.0.0.1",
        "SocketPath": "/tmp/web.sock",
        "TaggedAddresses": {"lan": {"Address": "10.0.0.1", "Port": 8080}},
        "Weights": {"Passing": 1, "Warning": 1},
        "EnableTagOverride": false,
        "CreateIndex": 7,
        "ModifyIndex": 9,
        "Proxy": {"Config": {}},
        "Connect": {},
        "PeerName": "peer-1",
        "AI": {"Models": []},
        "Datacenter": "dc1",
        "Namespace": "default",
        "Partition": "default",
        "Locality": {"Region": "us-east-1"}
    }))
    .unwrap();
    assert_eq!(svc.id, "web-1");
    assert_eq!(svc.kind, "typical");
    assert_eq!(svc.socket_path, "/tmp/web.sock");
    assert_eq!(svc.peer_name, "peer-1");
    assert_eq!(svc.create_index, 7);
    assert_eq!(svc.modify_index, 9);
    assert_eq!(svc.ports.as_ref().unwrap()["http"], 8080);
    assert_eq!(svc.tagged_addresses.as_ref().unwrap()["lan"]["Port"], 8080);
    assert!(svc.proxy.is_some());
    assert!(svc.connect.is_some());
    assert!(svc.locality.is_some());
    assert_eq!(svc.ai, Some(json!({"Models": []})));

    let roundtrip = serde_json::to_value(&svc).unwrap();
    assert!(roundtrip.get("AI").is_some());
    assert!(roundtrip.get("Ai").is_none());
}

#[test]
fn agent_check_deserializes_upstream_payload() {
    let check: AgentCheck = serde_json::from_value(json!({
        "Node": "node1",
        "CheckID": "web-check",
        "Name": "web health",
        "Status": "passing",
        "Notes": "",
        "Output": "ok",
        "ServiceID": "web-1",
        "ServiceName": "web",
        "Type": "http",
        "ExposedPort": 9090,
        "Definition": {"HTTP": "http://localhost/health"},
        "CreateIndex": 3,
        "ModifyIndex": 4
    }))
    .unwrap();
    assert_eq!(check.check_id, "web-check");
    assert_eq!(check.exposed_port, 9090);
    assert_eq!(check.definition.unwrap()["HTTP"], "http://localhost/health");
}

#[test]
fn health_check_deserializes_upstream_payload() {
    let check: HealthCheck = serde_json::from_value(json!({
        "Node": "node1",
        "CheckID": "web-check",
        "Name": "web health",
        "Status": "passing",
        "Notes": "",
        "Output": "ok",
        "ServiceID": "web-1",
        "ServiceName": "web",
        "ServiceTags": ["v1"],
        "Type": "http",
        "ExposedPort": 9090,
        "PeerName": "peer-1",
        "Definition": {"Interval": "10s"},
        "CreateIndex": 3,
        "ModifyIndex": 4
    }))
    .unwrap();
    assert_eq!(check.exposed_port, 9090);
    assert_eq!(check.peer_name, "peer-1");
    assert_eq!(check.definition.unwrap()["Interval"], "10s");
    assert_eq!(check.service_tags.unwrap()[0], "v1");
}

#[test]
fn agent_service_check_serializes_upstream_acronym_keys() {
    let check = AgentServiceCheck {
        script_args: Some(vec!["/bin/check".into()]),
        docker_container_id: Some("abc123".into()),
        shell: Some("/bin/sh".into()),
        header: Some(
            [("X-Token".to_string(), vec!["secret".to_string()])]
                .into_iter()
                .collect(),
        ),
        method: Some("POST".into()),
        body: Some("ping".into()),
        tcp_use_tls: Some(true),
        udp: Some("127.0.0.1:53".into()),
        tls_server_name: Some("example.com".into()),
        h2ping: Some("example.com".into()),
        h2ping_use_tls: Some(true),
        alias_node: Some("alias-node".into()),
        alias_service: Some("alias-svc".into()),
        success_before_passing: Some(2),
        failures_before_warning: Some(3),
        failures_before_critical: Some(4),
        ..Default::default()
    };
    let obj = serde_json::to_value(&check).unwrap();
    assert_eq!(obj["ScriptArgs"][0], "/bin/check");
    assert_eq!(obj["DockerContainerID"], "abc123");
    assert_eq!(obj["TCPUseTLS"], true);
    assert_eq!(obj["TLSServerName"], "example.com");
    assert_eq!(obj["H2PING"], "example.com");
    assert_eq!(obj["H2PingUseTLS"], true);
    assert_eq!(obj["SuccessBeforePassing"], 2);
    assert_eq!(obj["FailuresBeforeWarning"], 3);
    assert_eq!(obj["FailuresBeforeCritical"], 4);
    assert_eq!(obj["Header"]["X-Token"][0], "secret");
    for wrong in ["DockerContainerId", "TcpUseTLS", "TlsServerName", "H2Ping"] {
        assert!(
            !format!("{obj}").contains(&format!("\"{wrong}\"")),
            "wrong local spelling {wrong} leaked into wire JSON"
        );
    }

    let parsed: AgentServiceCheck = serde_json::from_value(json!({
        "DockerContainerId": "legacy",
        "TcpUseTLS": false,
        "TlsServerName": "legacy.example",
        "H2Ping": "legacy-h2"
    }))
    .unwrap();
    assert_eq!(parsed.docker_container_id.as_deref(), Some("legacy"));
    assert_eq!(parsed.tcp_use_tls, Some(false));
    assert_eq!(parsed.tls_server_name.as_deref(), Some("legacy.example"));
    assert_eq!(parsed.h2ping.as_deref(), Some("legacy-h2"));
}

#[test]
fn catalog_service_deserializes_upstream_payload() {
    let svc: CatalogService = serde_json::from_value(json!({
        "ID": "node1",
        "Node": "node1",
        "Address": "10.0.0.1",
        "ServiceID": "web-1",
        "ServiceName": "web",
        "ServiceAddress": "10.0.0.2",
        "ServicePort": 8080,
        "ServiceEnableTagOverride": false,
        "ServiceTaggedAddresses": {"lan": {"Address": "10.0.0.2", "Port": 8080}},
        "ServiceProxy": {"Config": {}},
        "ServiceLocality": {"Region": "us-east-1"},
        "Checks": {"web-check": {
            "Node": "node1", "CheckID": "web-check", "Name": "web health",
            "Status": "passing", "Notes": "", "Output": "ok",
            "ServiceID": "web-1", "ServiceName": "web",
            "CreateIndex": 3, "ModifyIndex": 4
        }},
        "CreateIndex": 7,
        "ModifyIndex": 9
    }))
    .unwrap();
    assert_eq!(svc.service_tagged_addresses.unwrap()["lan"]["Port"], 8080);
    assert!(svc.service_proxy.is_some());
    assert!(svc.service_locality.is_some());
    let checks = svc.checks.unwrap();
    assert_eq!(checks["web-check"].status, "passing");
}

#[test]
fn node_and_catalog_registration_serialize_upstream_keys() {
    let node: Node = serde_json::from_value(json!({
        "Node": "node1",
        "Address": "10.0.0.1",
        "Partition": "default",
        "PeerName": "peer-1",
        "Locality": {"Region": "us-east-1"}
    }))
    .unwrap();
    assert_eq!(node.partition, "default");
    assert_eq!(node.peer_name, "peer-1");
    assert!(node.locality.is_some());

    let reg = CatalogRegistration {
        node: "node1".into(),
        address: "10.0.0.1".into(),
        skip_node_update: true,
        partition: Some("default".into()),
        locality: Some(json!({"Region": "us-east-1"})),
        ..Default::default()
    };
    let obj = serde_json::to_value(&reg).unwrap();
    assert_eq!(obj["SkipNodeUpdate"], true);
    assert_eq!(obj["Partition"], "default");
    assert_eq!(obj["Locality"]["Region"], "us-east-1");
}

#[test]
fn ingress_service_deserializes_upstream_limits() {
    let svc: IngressService = serde_json::from_value(json!({
        "Name": "web",
        "Partition": "default",
        "MaxConnections": 100,
        "MaxPendingRequests": 200,
        "MaxConcurrentRequests": 300,
        "PassiveHealthCheck": {"MaxFailures": 3}
    }))
    .unwrap();
    assert_eq!(svc.partition, "default");
    assert_eq!(svc.max_connections, Some(100));
    assert_eq!(svc.max_pending_requests, Some(200));
    assert_eq!(svc.max_concurrent_requests, Some(300));
    assert_eq!(svc.passive_health_check.unwrap()["MaxFailures"], 3);
}
