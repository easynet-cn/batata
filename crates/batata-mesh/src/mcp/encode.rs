//! Wire encoding for the Istio MCP `ServiceEntry` collection.
//!
//! The native types in [`super::types`] are configuration DTOs. Istio's MCP
//! sink expects protobuf: a `Resources` stream message containing MCP
//! `Resource` wrappers, each with `Metadata` and a `google.protobuf.Any` body
//! holding an Istio `ServiceEntry`. This module performs that translation
//! without changing the native model.

use prost::Message;

use crate::xds::generated::{istio, mcp};

use super::types::{Location, Protocol, Resolution, ServiceEntry};

/// MCP collection served by upstream Nacos on port 18848.
pub const SERVICE_ENTRY_COLLECTION: &str = "istio/networking/v1alpha3/serviceentries";

/// `google.protobuf.Any.type_url` for an Istio `ServiceEntry` body.
pub const SERVICE_ENTRY_TYPE_URL: &str =
    "type.googleapis.com/istio.networking.v1alpha3.ServiceEntry";

/// Encode one native `ServiceEntry` DTO into the Istio wire message.
pub fn encode_service_entry(entry: &ServiceEntry) -> istio::ServiceEntry {
    istio::ServiceEntry {
        hosts: entry.spec.hosts.clone(),
        addresses: entry.spec.addresses.clone(),
        ports: entry
            .spec
            .ports
            .iter()
            .map(|port| istio::Port {
                number: port.number,
                protocol: protocol_name(port.protocol).to_string(),
                name: port.name.clone(),
                target_port: port.target_port.unwrap_or(0),
            })
            .collect(),
        location: match entry.spec.location {
            Location::MeshInternal => istio::service_entry::Location::MeshInternal as i32,
            Location::MeshExternal => istio::service_entry::Location::MeshExternal as i32,
        },
        resolution: match entry.spec.resolution {
            Resolution::None => istio::service_entry::Resolution::None as i32,
            Resolution::Static => istio::service_entry::Resolution::Static as i32,
            // Upstream v1alpha3 has no DNS_ROUND_ROBIN value; DNS is the
            // wire-compatible fallback for both DNS modes.
            Resolution::Dns | Resolution::DnsRoundRobin => {
                istio::service_entry::Resolution::Dns as i32
            }
        },
        endpoints: entry
            .spec
            .endpoints
            .iter()
            .map(|endpoint| istio::WorkloadEntry {
                address: endpoint.address.clone().unwrap_or_default(),
                ports: endpoint.ports.clone(),
                labels: endpoint.labels.clone(),
                network: endpoint.network.clone().unwrap_or_default(),
                locality: endpoint.locality.clone().unwrap_or_default(),
                weight: endpoint.weight.unwrap_or(0),
                service_account: endpoint.service_account.clone().unwrap_or_default(),
            })
            .collect(),
        export_to: entry.spec.export_to.clone(),
        subject_alt_names: entry.spec.subject_alt_names.clone(),
    }
}

/// Encode one native `ServiceEntry` DTO into an MCP `Resource` wrapper.
pub fn encode_resource(entry: &ServiceEntry) -> mcp::Resource {
    let mut labels = entry.metadata.labels.clone();
    labels
        .entry("registryType".to_string())
        .or_insert_with(|| "nacos".to_string());

    let mut annotations = entry.metadata.annotations.clone();
    annotations
        .entry("virtual".to_string())
        .or_insert_with(|| "1".to_string());

    let name = if entry.metadata.namespace.is_empty() {
        entry.metadata.name.clone()
    } else {
        format!("{}/{}", entry.metadata.namespace, entry.metadata.name)
    };

    mcp::Resource {
        metadata: Some(mcp::Metadata {
            name,
            version: entry.metadata.resource_version.clone(),
            labels,
            annotations,
        }),
        body: Some(prost_types::Any {
            type_url: SERVICE_ENTRY_TYPE_URL.to_string(),
            value: encode_service_entry(entry).encode_to_vec(),
        }),
    }
}

/// Build a full-state MCP `Resources` response.
///
/// Upstream Nacos ignores the sink's `incremental` request and always sends
/// the whole collection, so `removed_resources` and `incremental` stay empty.
pub fn encode_resources(entries: &[ServiceEntry], version: u64, nonce: String) -> mcp::Resources {
    let mut sorted: Vec<ServiceEntry> = entries.to_vec();
    sorted.sort_by(|a, b| a.metadata.name.cmp(&b.metadata.name));

    mcp::Resources {
        system_version_info: version.to_string(),
        collection: SERVICE_ENTRY_COLLECTION.to_string(),
        resources: sorted.iter().map(encode_resource).collect(),
        removed_resources: Vec::new(),
        nonce,
        incremental: false,
    }
}

/// Map a native protocol to the Istio uppercase wire spelling.
fn protocol_name(protocol: Protocol) -> &'static str {
    match protocol {
        Protocol::Http => "HTTP",
        Protocol::Https => "HTTPS",
        Protocol::Grpc => "GRPC",
        Protocol::GrpcWeb => "GRPC-WEB",
        Protocol::Http2 => "HTTP2",
        Protocol::Mongo => "MONGO",
        Protocol::Mysql => "MYSQL",
        Protocol::Redis => "REDIS",
        Protocol::Tcp => "TCP",
        Protocol::Tls => "TLS",
        Protocol::Udp => "UDP",
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::mcp::types::{ServiceEntrySpec, WorkloadEntry};

    fn sample_entry() -> ServiceEntry {
        ServiceEntry {
            metadata: super::super::types::ResourceMetadata {
                name: "web".to_string(),
                namespace: "public".to_string(),
                resource_version: "7".to_string(),
                ..Default::default()
            },
            spec: ServiceEntrySpec {
                hosts: vec!["web.nacos".to_string()],
                ports: vec![super::super::types::Port {
                    number: 8080,
                    name: "http".to_string(),
                    protocol: Protocol::Grpc,
                    target_port: Some(9090),
                }],
                resolution: Resolution::Static,
                location: Location::MeshInternal,
                endpoints: vec![WorkloadEntry {
                    address: Some("127.0.0.1".to_string()),
                    ports: HashMap::from([("http".to_string(), 8080)]),
                    locality: Some("region/zone".to_string()),
                    weight: Some(10),
                    ..Default::default()
                }],
                export_to: vec!["*".to_string()],
                ..Default::default()
            },
        }
    }

    #[test]
    fn service_entry_wire_fields_match_upstream() {
        let wire = encode_service_entry(&sample_entry());
        let bytes = prost::Message::encode_to_vec(&wire);
        let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        // Regression anchor for upstream field numbers: ServiceEntry
        // hosts=1, ports=3, location=4, resolution=5, endpoints=6,
        // export_to=7; Port number=1/protocol=2/name=3/target_port=4;
        // WorkloadEntry address=1/ports=2/locality=5/weight=6.
        assert_eq!(
            hex,
            "0a097765622e6e61636f731a1208903f1204475250431a04687474702082472001280132250a093132372e302e302e3112090a046874747010903f2a0b726567696f6e2f7a6f6e65300a3a012a"
        );

        assert_eq!(wire.hosts, vec!["web.nacos"]);
        assert_eq!(wire.ports[0].number, 8080);
        assert_eq!(wire.ports[0].protocol, "GRPC");
        assert_eq!(wire.ports[0].name, "http");
        assert_eq!(wire.ports[0].target_port, 9090);
        assert_eq!(
            wire.location,
            istio::service_entry::Location::MeshInternal as i32
        );
        assert_eq!(
            wire.resolution,
            istio::service_entry::Resolution::Static as i32
        );
        assert_eq!(wire.endpoints[0].address, "127.0.0.1");
        assert_eq!(wire.endpoints[0].locality, "region/zone");
        assert_eq!(wire.endpoints[0].weight, 10);
        assert_eq!(wire.export_to, vec!["*"]);
    }

    #[test]
    fn resource_wrapper_uses_mcp_metadata_and_any_body() {
        let resource = encode_resource(&sample_entry());

        let metadata = resource.metadata.expect("metadata");
        assert_eq!(metadata.name, "public/web");
        assert_eq!(metadata.version, "7");
        assert_eq!(
            metadata.labels.get("registryType"),
            Some(&"nacos".to_string())
        );
        assert_eq!(metadata.annotations.get("virtual"), Some(&"1".to_string()));

        let body = resource.body.expect("any body");
        assert_eq!(
            body.type_url,
            "type.googleapis.com/istio.networking.v1alpha3.ServiceEntry"
        );
        let decoded = istio::ServiceEntry::decode(body.value.as_slice()).expect("decode body");
        assert_eq!(decoded.hosts, vec!["web.nacos"]);
    }

    #[test]
    fn resources_response_is_full_state_and_sorted() {
        let mut first = sample_entry();
        first.metadata.name = "zeta".to_string();
        let mut second = sample_entry();
        second.metadata.name = "alpha".to_string();

        let response = encode_resources(&[first, second], 42, "nonce".to_string());

        assert_eq!(
            response.collection,
            "istio/networking/v1alpha3/serviceentries"
        );
        assert_eq!(response.system_version_info, "42");
        assert_eq!(response.nonce, "nonce");
        assert!(!response.incremental);
        assert!(response.removed_resources.is_empty());
        assert_eq!(response.resources.len(), 2);
        assert_eq!(
            response.resources[0]
                .metadata
                .as_ref()
                .expect("metadata")
                .name,
            "public/alpha"
        );
    }
}
