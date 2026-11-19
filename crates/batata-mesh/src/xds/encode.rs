//! Encoding of internal xDS resource models into Envoy v3 protobuf wire format.
//!
//! Historically resources were serialized with `serde_json` and placed into
//! `google.protobuf.Any.value`. That is not a valid Envoy protobuf payload: a
//! real Envoy sidecar would reject the discovery response during `Any`
//! unpacking. This module converts the native Rust models in
//! [`crate::xds::types`] into the generated prost messages produced from
//! `proto/*.proto` and returns their encoded bytes.
//!
//! The `.proto` files are hand-written minimal subsets of the upstream Envoy
//! API, not vendored copies. Only fields actually emitted here are declared,
//! and their field numbers are verified against upstream `envoyproxy/envoy`;
//! see `docs/compat/review-report.md` §34.4 / §36. Emitting a wrong-typed or
//! JSON `Any.value` is worse than omitting a resource, so unsupported filter
//! kinds, path matchers and route actions are skipped rather than guessed.

use prost::Message;

use crate::xds::ResourceType;
use crate::xds::generated::{
    cluster as cds, endpoint as eds, http_connection_manager as hcm, listener as lds, route as rds,
    tcp_proxy as tcp, tls,
};
use crate::xds::type_urls;
use crate::xds::types::{
    CidrRange as NativeCidrRange, Cluster, ClusterLoadAssignment, DiscoveryType, Endpoint,
    FilterChain, FilterChainMatch, HeaderMatchType, HeaderMatcher, HealthStatus, HttpCodecType,
    LbPolicy, Listener, ListenerAddress, ListenerProtocol, Locality, LocalityEndpoints,
    NetworkFilter, NetworkFilterType, PathMatch, Route, RouteAction, RouteConfiguration,
    RouteDestination, RouteMatch, SdsSecret, SdsSecretKind,
    TransportSocket as NativeTransportSocket, TransportSocketConfig, VirtualHost, WeightedCluster,
};

/// Envoy network filter name for the HTTP connection manager.
pub const HCM_FILTER_NAME: &str = "envoy.filters.network.http_connection_manager";
/// Envoy network filter name for the TCP proxy.
pub const TCP_PROXY_FILTER_NAME: &str = "envoy.filters.network.tcp_proxy";
/// Envoy terminal HTTP filter required at the end of an HCM filter chain.
pub const ROUTER_FILTER_NAME: &str = "envoy.filters.http.router";

/// Encode a resource into the bytes stored in `google.protobuf.Any.value`.
///
/// Returns `None` when a resource cannot be represented as a valid Envoy
/// protobuf (Runtime is still unsupported; a listener with no encodable
/// filter is also skipped). Callers must skip those instead of emitting an
/// invalid `Any` payload, which would make Envoy NACK the whole response.
pub fn encode_resource(resource_type: ResourceType, native: NativeResource<'_>) -> Option<Vec<u8>> {
    match (resource_type, native) {
        (ResourceType::Cluster, NativeResource::Cluster(c)) => Some(encode_cluster(c)),
        (ResourceType::Endpoint, NativeResource::Endpoints(c)) => Some(encode_endpoints(c)),
        (ResourceType::Listener, NativeResource::Listener(l)) => encode_listener(l),
        (ResourceType::Route, NativeResource::Route(r)) => Some(encode_route(r)),
        (ResourceType::Secret, NativeResource::Secret(s)) => Some(encode_secret(s)),
        _ => None,
    }
}

/// Borrowed native resource variants accepted by [`encode_resource`].
#[derive(Debug, Clone, Copy)]
pub enum NativeResource<'a> {
    /// A CDS cluster.
    Cluster(&'a Cluster),
    /// An EDS cluster load assignment.
    Endpoints(&'a ClusterLoadAssignment),
    /// An LDS listener.
    Listener(&'a Listener),
    /// An RDS route configuration.
    Route(&'a RouteConfiguration),
    /// An SDS secret.
    Secret(&'a SdsSecret),
}

/// Encode a native [`Cluster`] as `envoy.config.cluster.v3.Cluster`.
pub fn encode_cluster(c: &Cluster) -> Vec<u8> {
    let discovery_type = match c.discovery_type {
        DiscoveryType::Static => cds::cluster::DiscoveryType::Static,
        DiscoveryType::StrictDns => cds::cluster::DiscoveryType::StrictDns,
        DiscoveryType::LogicalDns => cds::cluster::DiscoveryType::LogicalDns,
        DiscoveryType::Eds => cds::cluster::DiscoveryType::Eds,
        DiscoveryType::OriginalDst => cds::cluster::DiscoveryType::OriginalDst,
    };

    let lb_policy = match c.lb_policy {
        LbPolicy::RoundRobin => cds::cluster::LbPolicy::RoundRobin,
        LbPolicy::LeastRequest => cds::cluster::LbPolicy::LeastRequest,
        LbPolicy::RingHash => cds::cluster::LbPolicy::RingHash,
        LbPolicy::Random => cds::cluster::LbPolicy::Random,
        LbPolicy::Maglev => cds::cluster::LbPolicy::Maglev,
    };

    let (seconds, nanos) = duration_from_millis(c.connect_timeout_ms);

    // EDS clusters must point at ADS as their config source; otherwise Envoy
    // will never subscribe to the corresponding ClusterLoadAssignment.
    let eds_cluster_config =
        (discovery_type == cds::cluster::DiscoveryType::Eds).then(|| cds::EdsClusterConfig {
            eds_config: Some(cds::ConfigSource {
                ads: Some(cds::AggregatedConfigSource {}),
                resource_api_version: cds::ApiVersion::V3 as i32,
                ..Default::default()
            }),
            service_name: String::new(),
        });

    let message = cds::Cluster {
        name: c.name.clone(),
        r#type: discovery_type as i32,
        eds_cluster_config,
        connect_timeout: Some(::prost_types::Duration { seconds, nanos }),
        lb_policy: lb_policy as i32,
        transport_socket: c.transport_socket.as_ref().map(encode_cds_transport_socket),
        ..Default::default()
    };

    message.encode_to_vec()
}

/// Decode an `envoy.config.cluster.v3.Cluster` from `Any.value` bytes.
///
/// Used by tests and by tooling that needs to inspect what would be sent to a
/// real Envoy proxy.
pub fn decode_cluster(bytes: &[u8]) -> Result<cds::Cluster, prost::DecodeError> {
    cds::Cluster::decode(bytes)
}

/// Encode a native [`SdsSecret`] as an Envoy TLS `Secret` resource.
pub fn encode_secret(secret: &SdsSecret) -> Vec<u8> {
    let kind = match &secret.kind {
        SdsSecretKind::TlsCertificate {
            certificate_chain,
            private_key,
        } => tls::secret::Type::TlsCertificate(tls::TlsCertificate {
            certificate_chain: Some(inline_string(certificate_chain)),
            private_key: Some(inline_string(private_key)),
        }),
        SdsSecretKind::ValidationContext { trusted_ca } => {
            tls::secret::Type::ValidationContext(tls::CertificateValidationContext {
                trusted_ca: Some(inline_string(trusted_ca)),
                ..Default::default()
            })
        }
    };
    tls::Secret {
        name: secret.name.clone(),
        r#type: Some(kind),
    }
    .encode_to_vec()
}

/// Decode an `envoy.extensions.transport_sockets.tls.v3.Secret` from `Any.value` bytes.
pub fn decode_secret(bytes: &[u8]) -> Result<tls::Secret, prost::DecodeError> {
    tls::Secret::decode(bytes)
}

fn inline_string(value: &str) -> tls::DataSource {
    tls::DataSource {
        specifier: Some(tls::data_source::Specifier::InlineString(value.to_string())),
    }
}

fn ads_config_source() -> tls::ConfigSource {
    tls::ConfigSource {
        config_source_specifier: Some(tls::config_source::ConfigSourceSpecifier::Ads(
            tls::AggregatedConfigSource {},
        )),
        resource_api_version: tls::ApiVersion::V3 as i32,
    }
}

/// Encode a `DownstreamTlsContext` typed_config for a listener transport socket.
pub fn encode_downstream_tls_context(
    tls_certificate_secret_name: &str,
    validation_context_secret_name: &str,
    require_client_certificate: bool,
    alpn_protocols: &[String],
) -> Vec<u8> {
    tls::DownstreamTlsContext {
        common_tls_context: Some(tls::CommonTlsContext {
            tls_certificates: Vec::new(),
            validation_context: None,
            alpn_protocols: alpn_protocols.to_vec(),
            tls_certificate_sds_secret_configs: vec![tls::SdsSecretConfig {
                name: tls_certificate_secret_name.to_string(),
                sds_config: Some(ads_config_source()),
            }],
            validation_context_sds_secret_config: Some(tls::SdsSecretConfig {
                name: validation_context_secret_name.to_string(),
                sds_config: Some(ads_config_source()),
            }),
        }),
        require_client_certificate: Some(require_client_certificate),
    }
    .encode_to_vec()
}

/// Encode an `UpstreamTlsContext` typed_config for a cluster transport socket.
pub fn encode_upstream_tls_context(
    certificate_chain: &str,
    private_key: &str,
    trusted_ca: &str,
    san_uri: &str,
    sni: &str,
) -> Vec<u8> {
    let mut validation_context = tls::CertificateValidationContext {
        trusted_ca: Some(inline_string(trusted_ca)),
        ..Default::default()
    };
    // "Skip rather than guess": no SAN matcher is emitted for an empty URI.
    if !san_uri.is_empty() {
        validation_context
            .match_typed_subject_alt_names
            .push(tls::SubjectAltNameMatcher {
                san_type: tls::SanType::Uri as i32,
                matcher: Some(tls::StringMatcher {
                    match_pattern: Some(tls::string_matcher::MatchPattern::Exact(
                        san_uri.to_string(),
                    )),
                }),
            });
    }
    tls::UpstreamTlsContext {
        common_tls_context: Some(tls::CommonTlsContext {
            tls_certificates: vec![tls::TlsCertificate {
                certificate_chain: Some(inline_string(certificate_chain)),
                private_key: Some(inline_string(private_key)),
            }],
            validation_context: Some(validation_context),
            alpn_protocols: Vec::new(),
            tls_certificate_sds_secret_configs: Vec::new(),
            validation_context_sds_secret_config: None,
        }),
        sni: sni.to_string(),
    }
    .encode_to_vec()
}

/// Build the `Any` carried by a cluster / filter chain transport socket.
fn transport_socket_typed_config(socket: &NativeTransportSocket) -> ::prost_types::Any {
    let (type_url, value) = match &socket.config {
        TransportSocketConfig::DownstreamSds {
            tls_certificate_secret_name,
            validation_context_secret_name,
            require_client_certificate,
            alpn_protocols,
        } => (
            type_urls::DOWNSTREAM_TLS_CONTEXT_TYPE,
            encode_downstream_tls_context(
                tls_certificate_secret_name,
                validation_context_secret_name,
                *require_client_certificate,
                alpn_protocols,
            ),
        ),
        TransportSocketConfig::UpstreamInline {
            certificate_chain,
            private_key,
            trusted_ca,
            san_uri,
            sni,
        } => (
            type_urls::UPSTREAM_TLS_CONTEXT_TYPE,
            encode_upstream_tls_context(certificate_chain, private_key, trusted_ca, san_uri, sni),
        ),
    };
    ::prost_types::Any {
        type_url: type_url.to_string(),
        value,
    }
}

fn encode_cds_transport_socket(socket: &NativeTransportSocket) -> cds::TransportSocket {
    cds::TransportSocket {
        name: socket.name.clone(),
        typed_config: Some(transport_socket_typed_config(socket)),
    }
}

fn encode_lds_transport_socket(socket: &NativeTransportSocket) -> lds::TransportSocket {
    lds::TransportSocket {
        name: socket.name.clone(),
        typed_config: Some(transport_socket_typed_config(socket)),
    }
}

/// Encode a native [`ClusterLoadAssignment`] as
/// `envoy.config.endpoint.v3.ClusterLoadAssignment`.
pub fn encode_endpoints(cla: &ClusterLoadAssignment) -> Vec<u8> {
    let endpoints = cla
        .endpoints
        .iter()
        .map(encode_locality_endpoints)
        .collect::<Vec<_>>();

    let message = eds::ClusterLoadAssignment {
        cluster_name: cla.cluster_name.clone(),
        endpoints,
        policy: None,
    };

    message.encode_to_vec()
}

/// Decode an `envoy.config.endpoint.v3.ClusterLoadAssignment` from bytes.
pub fn decode_endpoints(bytes: &[u8]) -> Result<eds::ClusterLoadAssignment, prost::DecodeError> {
    eds::ClusterLoadAssignment::decode(bytes)
}

fn encode_locality_endpoints(group: &LocalityEndpoints) -> eds::LocalityLbEndpoints {
    eds::LocalityLbEndpoints {
        locality: Some(encode_locality(&group.locality)),
        lb_endpoints: group.endpoints.iter().map(encode_lb_endpoint).collect(),
        load_balancing_weight: Some(group.weight.max(1)),
        proximity: None,
        priority: group.priority,
    }
}

fn encode_lb_endpoint(ep: &Endpoint) -> eds::LbEndpoint {
    eds::LbEndpoint {
        host_identifier: Some(eds::lb_endpoint::HostIdentifier::Endpoint(eds::Endpoint {
            address: Some(eds::Address {
                address: Some(eds::address::Address::SocketAddress(eds::SocketAddress {
                    protocol: eds::socket_address::Protocol::Tcp as i32,
                    address: ep.address.ip().to_string(),
                    resolver_name: String::new(),
                    ipv4_compat: false,
                    port_specifier: Some(eds::socket_address::PortSpecifier::PortValue(u32::from(
                        ep.address.port(),
                    ))),
                })),
            }),
            health_check_config: None,
            hostname: String::new(),
        })),
        health_status: encode_health_status(&ep.health_status) as i32,
        metadata: None,
        load_balancing_weight: Some(ep.weight.max(1)),
    }
}

fn encode_health_status(status: &HealthStatus) -> eds::HealthStatus {
    match status {
        HealthStatus::Unknown => eds::HealthStatus::Unknown,
        HealthStatus::Healthy => eds::HealthStatus::Healthy,
        HealthStatus::Unhealthy => eds::HealthStatus::Unhealthy,
        HealthStatus::Draining => eds::HealthStatus::Draining,
        HealthStatus::Timeout => eds::HealthStatus::Timeout,
        HealthStatus::Degraded => eds::HealthStatus::Degraded,
    }
}

fn encode_locality(locality: &Locality) -> eds::Locality {
    eds::Locality {
        region: locality.region.clone(),
        zone: locality.zone.clone(),
        sub_zone: locality.sub_zone.clone(),
    }
}

/// Encode a native [`Listener`] as `envoy.config.listener.v3.Listener`.
///
/// Returns `None` when no filter chain contains an encodable network filter.
/// A listener with an empty filter chain would close every connection, so it is
/// better to skip it entirely than to send a structurally valid but useless LDS
/// resource.
pub fn encode_listener(listener: &Listener) -> Option<Vec<u8>> {
    let filter_chains: Vec<lds::FilterChain> = listener
        .filter_chains
        .iter()
        .filter_map(encode_filter_chain)
        .collect();
    let default_filter_chain = listener
        .default_filter_chain
        .as_ref()
        .and_then(encode_filter_chain);

    let has_filters = filter_chains.iter().any(|c| !c.filters.is_empty())
        || default_filter_chain
            .as_ref()
            .is_some_and(|c| !c.filters.is_empty());
    if !has_filters {
        return None;
    }

    let message = lds::Listener {
        name: listener.name.clone(),
        address: Some(encode_listener_address(&listener.address)),
        filter_chains,
        default_filter_chain,
    };
    Some(message.encode_to_vec())
}

/// Decode an `envoy.config.listener.v3.Listener` from `Any.value` bytes.
pub fn decode_listener(bytes: &[u8]) -> Result<lds::Listener, prost::DecodeError> {
    lds::Listener::decode(bytes)
}

fn encode_listener_address(address: &ListenerAddress) -> lds::Address {
    lds::Address {
        address: Some(lds::address::Address::SocketAddress(lds::SocketAddress {
            protocol: match address.protocol {
                ListenerProtocol::Tcp => lds::socket_address::Protocol::Tcp as i32,
                ListenerProtocol::Udp => lds::socket_address::Protocol::Udp as i32,
            },
            address: address.address.clone(),
            resolver_name: String::new(),
            ipv4_compat: false,
            port_specifier: Some(lds::socket_address::PortSpecifier::PortValue(u32::from(
                address.port,
            ))),
        })),
    }
}

fn encode_filter_chain(chain: &FilterChain) -> Option<lds::FilterChain> {
    let filters = chain
        .filters
        .iter()
        .filter_map(encode_network_filter)
        .collect::<Vec<_>>();
    if filters.is_empty() {
        return None;
    }
    Some(lds::FilterChain {
        name: chain.name.clone(),
        filter_chain_match: chain
            .filter_chain_match
            .as_ref()
            .map(encode_filter_chain_match),
        filters,
        transport_socket: chain
            .transport_socket
            .as_ref()
            .map(encode_lds_transport_socket),
    })
}

fn encode_filter_chain_match(m: &FilterChainMatch) -> lds::FilterChainMatch {
    lds::FilterChainMatch {
        destination_port: m.destination_port.map(u32::from),
        prefix_ranges: m.prefix_ranges.iter().map(encode_cidr_range).collect(),
        transport_protocol: m.transport_protocol.clone().unwrap_or_default(),
        application_protocols: m.application_protocols.clone(),
        server_names: m.server_names.clone(),
    }
}

fn encode_cidr_range(c: &NativeCidrRange) -> lds::CidrRange {
    lds::CidrRange {
        address_prefix: c.address_prefix.clone(),
        prefix_len: Some(c.prefix_len),
    }
}

fn encode_network_filter(filter: &NetworkFilter) -> Option<lds::Filter> {
    let (type_url, value) = match &filter.filter_type {
        NetworkFilterType::HttpConnectionManager {
            route_config_name,
            codec_type,
            stat_prefix,
            rbac_config,
        } => (
            type_urls::HCM_TYPE,
            encode_http_connection_manager(
                route_config_name,
                *codec_type,
                stat_prefix,
                rbac_config.as_deref(),
            ),
        ),
        NetworkFilterType::TcpProxy {
            stat_prefix,
            cluster,
        } => (
            type_urls::TCP_PROXY_TYPE,
            encode_tcp_proxy(stat_prefix, cluster),
        ),
        NetworkFilterType::Rbac { config } => (type_urls::NETWORK_RBAC_TYPE, config.clone()),
        // A generic JSON blob is not a valid Envoy protobuf payload.
        NetworkFilterType::Generic { .. } => return None,
    };

    Some(lds::Filter {
        name: filter.name.clone(),
        config_type: Some(lds::filter::ConfigType::TypedConfig(::prost_types::Any {
            type_url: type_url.to_string(),
            value,
        })),
    })
}

/// Encode an HCM network filter config (`type_url` payload, not an xDS resource).
pub fn encode_http_connection_manager(
    route_config_name: &str,
    codec_type: HttpCodecType,
    stat_prefix: &str,
    rbac_config: Option<&[u8]>,
) -> Vec<u8> {
    let codec = match codec_type {
        HttpCodecType::Auto => hcm::CodecType::Auto,
        HttpCodecType::Http1 => hcm::CodecType::Http1,
        HttpCodecType::Http2 => hcm::CodecType::Http2,
        HttpCodecType::Http3 => hcm::CodecType::Http3,
    };

    let mut http_filters: Vec<hcm::HttpFilter> = Vec::new();

    // L7 RBAC (when present) must run before the router so an unauthorized
    // request is rejected before any upstream routing/forwarding happens.
    if let Some(config) = rbac_config {
        http_filters.push(hcm::HttpFilter {
            name: "envoy.filters.http.rbac".to_string(),
            config_type: Some(hcm::http_filter::ConfigType::TypedConfig(
                ::prost_types::Any {
                    type_url: type_urls::HTTP_RBAC_TYPE.to_string(),
                    value: config.to_vec(),
                },
            )),
        });
    }

    // Envoy requires the terminal router filter. Its config message has no
    // required fields, so an empty typed_config decodes to the default Router.
    http_filters.push(hcm::HttpFilter {
        name: ROUTER_FILTER_NAME.to_string(),
        config_type: Some(hcm::http_filter::ConfigType::TypedConfig(
            ::prost_types::Any {
                type_url: type_urls::ROUTER_FILTER_TYPE.to_string(),
                value: Vec::new(),
            },
        )),
    });

    hcm::HttpConnectionManager {
        codec_type: codec as i32,
        stat_prefix: stat_prefix.to_string(),
        http_filters,
        route_specifier: Some(hcm::http_connection_manager::RouteSpecifier::Rds(
            hcm::Rds {
                config_source: Some(hcm::ConfigSource {
                    ads: Some(hcm::AggregatedConfigSource {}),
                    resource_api_version: hcm::ApiVersion::V3 as i32,
                }),
                route_config_name: route_config_name.to_string(),
            },
        )),
    }
    .encode_to_vec()
}

/// Decode an HCM filter config from `Filter.typed_config.value`.
pub fn decode_http_connection_manager(
    bytes: &[u8],
) -> Result<hcm::HttpConnectionManager, prost::DecodeError> {
    hcm::HttpConnectionManager::decode(bytes)
}

/// Encode a TCP proxy network filter config.
pub fn encode_tcp_proxy(stat_prefix: &str, cluster: &str) -> Vec<u8> {
    tcp::TcpProxy {
        stat_prefix: stat_prefix.to_string(),
        cluster: cluster.to_string(),
    }
    .encode_to_vec()
}

/// Decode a TCP proxy filter config from `Filter.typed_config.value`.
pub fn decode_tcp_proxy(bytes: &[u8]) -> Result<tcp::TcpProxy, prost::DecodeError> {
    tcp::TcpProxy::decode(bytes)
}

/// Encode a native [`RouteConfiguration`] as
/// `envoy.config.route.v3.RouteConfiguration`.
///
/// Routes whose path matcher or action cannot be represented by the checked-in
/// proto subset are skipped. The resource itself is still valid protobuf.
pub fn encode_route(config: &RouteConfiguration) -> Vec<u8> {
    rds::RouteConfiguration {
        name: config.name.clone(),
        virtual_hosts: config
            .virtual_hosts
            .iter()
            .map(encode_virtual_host)
            .collect(),
        internal_only_headers: config.internal_only_headers.clone(),
    }
    .encode_to_vec()
}

/// Decode an `envoy.config.route.v3.RouteConfiguration` from `Any.value` bytes.
pub fn decode_route(bytes: &[u8]) -> Result<rds::RouteConfiguration, prost::DecodeError> {
    rds::RouteConfiguration::decode(bytes)
}

fn encode_virtual_host(vh: &VirtualHost) -> rds::VirtualHost {
    rds::VirtualHost {
        name: vh.name.clone(),
        domains: vh.domains.clone(),
        routes: vh.routes.iter().filter_map(encode_route_entry).collect(),
    }
}

fn encode_route_entry(route: &Route) -> Option<rds::Route> {
    let r#match = Some(encode_route_match(&route.match_config)?);
    let action = match &route.action {
        RouteAction::Route(dest) => encode_route_action(dest)?,
        // Redirect / direct-response bodies are not in the checked-in subset.
        RouteAction::Redirect(_) | RouteAction::DirectResponse(_) => return None,
    };
    Some(rds::Route {
        name: route.name.clone(),
        r#match,
        action: Some(action),
    })
}

fn encode_route_match(m: &RouteMatch) -> Option<rds::RouteMatch> {
    let path_specifier = match &m.path_match {
        PathMatch::Prefix(p) => Some(rds::route_match::PathSpecifier::Prefix(p.clone())),
        PathMatch::Path(p) => Some(rds::route_match::PathSpecifier::Path(p.clone())),
        // RegexMatcher / PathTemplate are not part of the checked-in subset;
        // emitting a prefix "/" instead would silently match every request.
        PathMatch::Regex(_) | PathMatch::PathTemplate(_) => return None,
    };
    Some(rds::RouteMatch {
        path_specifier,
        case_sensitive: None,
        headers: m.headers.iter().filter_map(encode_header_matcher).collect(),
    })
}

fn encode_header_matcher(h: &HeaderMatcher) -> Option<rds::HeaderMatcher> {
    let mut out = rds::HeaderMatcher {
        name: h.name.clone(),
        invert_match: h.invert_match,
        ..Default::default()
    };
    match &h.match_type {
        HeaderMatchType::Exact(v) => out.exact_match = v.clone(),
        HeaderMatchType::Prefix(v) => out.prefix_match = v.clone(),
        HeaderMatchType::Suffix(v) => out.suffix_match = v.clone(),
        HeaderMatchType::Present => out.present_match = true,
        // Regex / range matchers need `RegexMatcher` / `Int64Range` protos that
        // are not checked in; do not approximate them with a string match.
        HeaderMatchType::Regex(_) | HeaderMatchType::Range { .. } => return None,
    }
    Some(out)
}

fn encode_route_action(dest: &RouteDestination) -> Option<rds::route::Action> {
    let cluster_specifier = if !dest.weighted_clusters.is_empty() {
        let clusters = dest
            .weighted_clusters
            .iter()
            .map(encode_cluster_weight)
            .collect::<Vec<_>>();
        if clusters.is_empty() {
            return None;
        }
        let total_weight = clusters
            .iter()
            .map(|c| c.weight.unwrap_or(0))
            .sum::<u32>()
            .max(1);
        rds::route_action::ClusterSpecifier::WeightedClusters(rds::WeightedCluster {
            clusters,
            total_weight: Some(total_weight),
            ..Default::default()
        })
    } else if let Some(cluster) = &dest.cluster {
        rds::route_action::ClusterSpecifier::Cluster(cluster.clone())
    } else {
        return None;
    };

    let (seconds, nanos) = dest.timeout_ms.map(duration_from_millis).unwrap_or((0, 0));
    Some(rds::route::Action::Route(rds::RouteAction {
        cluster_specifier: Some(cluster_specifier),
        prefix_rewrite: dest.prefix_rewrite.clone().unwrap_or_default(),
        host_rewrite_literal: dest.host_rewrite.clone().unwrap_or_default(),
        timeout: dest
            .timeout_ms
            .map(|_| ::prost_types::Duration { seconds, nanos }),
    }))
}

fn encode_cluster_weight(w: &WeightedCluster) -> rds::ClusterWeight {
    rds::ClusterWeight {
        name: w.name.clone(),
        weight: Some(w.weight.max(1)),
    }
}

fn duration_from_millis(ms: u64) -> (i64, i32) {
    let seconds = (ms / 1000) as i64;
    let nanos = ((ms % 1000) * 1_000_000) as i32;
    (seconds, nanos)
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    use super::*;

    fn sample_endpoint(port: u16) -> Endpoint {
        Endpoint {
            address: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 7)), port),
            health_status: HealthStatus::Healthy,
            weight: 42,
            metadata: Default::default(),
            priority: 0,
        }
    }

    #[test]
    fn test_cluster_roundtrip_is_real_protobuf() {
        let mut cluster = Cluster::new_eds("payments");
        cluster.connect_timeout_ms = 2500;
        cluster.lb_policy = LbPolicy::LeastRequest;

        let bytes = encode_cluster(&cluster);
        let decoded = decode_cluster(&bytes).expect("must decode as envoy Cluster");

        assert_eq!(decoded.name, "payments");
        assert_eq!(decoded.r#type, cds::cluster::DiscoveryType::Eds as i32);
        assert_eq!(
            decoded.lb_policy,
            cds::cluster::LbPolicy::LeastRequest as i32
        );
        let timeout = decoded.connect_timeout.expect("connect timeout");
        assert_eq!(timeout.seconds, 2);
        assert_eq!(timeout.nanos, 500_000_000);
        let eds_config = decoded.eds_cluster_config.expect("eds config");
        assert!(eds_config.eds_config.expect("config source").ads.is_some());
    }

    #[test]
    fn test_endpoints_roundtrip_is_real_protobuf() {
        let mut cla = ClusterLoadAssignment::new("payments");
        cla.add_locality(
            Locality::new("region-a", "zone-1"),
            vec![sample_endpoint(8080), sample_endpoint(8081)],
            84,
        );

        let bytes = encode_endpoints(&cla);
        let decoded = decode_endpoints(&bytes).expect("must decode as envoy CLA");

        assert_eq!(decoded.cluster_name, "payments");
        assert_eq!(decoded.endpoints.len(), 1);
        let group = &decoded.endpoints[0];
        assert_eq!(group.locality.as_ref().unwrap().region, "region-a");
        assert_eq!(group.load_balancing_weight, Some(84));
        assert_eq!(group.lb_endpoints.len(), 2);
        let first = group.lb_endpoints[0]
            .host_identifier
            .as_ref()
            .expect("host identifier");
        match first {
            eds::lb_endpoint::HostIdentifier::Endpoint(ep) => {
                let addr = ep.address.as_ref().unwrap().address.as_ref().unwrap();
                match addr {
                    eds::address::Address::SocketAddress(sa) => {
                        assert_eq!(sa.address, "10.0.0.7");
                        assert_eq!(
                            sa.port_specifier,
                            Some(eds::socket_address::PortSpecifier::PortValue(8080))
                        );
                    }
                    _ => panic!("expected socket address"),
                }
            }
            _ => panic!("expected endpoint host identifier"),
        }
        assert_eq!(
            group.lb_endpoints[0].health_status,
            eds::HealthStatus::Healthy as i32
        );
    }

    #[test]
    fn test_listener_roundtrip_has_tcp_proxy_typed_config() {
        let filter_chain = FilterChain::new("inbound")
            .with_filter(NetworkFilter::tcp_proxy("payments"))
            .with_match(FilterChainMatch {
                destination_port: Some(21000),
                transport_protocol: Some("raw_buffer".to_string()),
                server_names: vec!["payments.consul".to_string()],
                ..Default::default()
            });
        let listener = Listener::new(
            "inbound_listener:21000",
            ListenerAddress::tcp("0.0.0.0", 21000),
        )
        .with_filter_chain(filter_chain);

        let bytes = encode_listener(&listener).expect("LDS must now be encodable");
        let decoded = decode_listener(&bytes).expect("must decode as envoy Listener");
        assert_eq!(decoded.name, "inbound_listener:21000");
        let addr = decoded.address.expect("address").address.expect("socket");
        match addr {
            lds::address::Address::SocketAddress(sa) => {
                assert_eq!(sa.address, "0.0.0.0");
                assert_eq!(
                    sa.port_specifier,
                    Some(lds::socket_address::PortSpecifier::PortValue(21000))
                );
            }
        }
        let chain = &decoded.filter_chains[0];
        assert_eq!(
            chain.filter_chain_match.as_ref().unwrap().destination_port,
            Some(21000)
        );
        let filter = &chain.filters[0];
        assert_eq!(filter.name, TCP_PROXY_FILTER_NAME);
        let lds::filter::ConfigType::TypedConfig(any) =
            filter.config_type.as_ref().expect("typed_config");
        assert_eq!(any.type_url, type_urls::TCP_PROXY_TYPE);
        let tcp = decode_tcp_proxy(&any.value).expect("must decode as TcpProxy");
        assert_eq!(tcp.cluster, "payments");
        assert_eq!(tcp.stat_prefix, "tcp_proxy");
    }

    #[test]
    fn test_listener_roundtrip_has_hcm_rds_reference() {
        let filter_chain = FilterChain::new("http")
            .with_filter(NetworkFilter::http_connection_manager("payments:routes"));
        let listener = Listener::new("http_listener:8080", ListenerAddress::tcp("0.0.0.0", 8080))
            .with_filter_chain(filter_chain);

        let bytes = encode_listener(&listener).expect("LDS must now be encodable");
        let decoded = decode_listener(&bytes).expect("must decode as envoy Listener");
        let lds::filter::ConfigType::TypedConfig(any) = decoded.filter_chains[0].filters[0]
            .config_type
            .as_ref()
            .expect("typed_config");
        assert_eq!(any.type_url, type_urls::HCM_TYPE);
        let hcm = decode_http_connection_manager(&any.value).expect("must decode as HCM");
        assert_eq!(hcm.stat_prefix, "ingress_http");
        assert_eq!(hcm.http_filters.len(), 1);
        assert_eq!(hcm.http_filters[0].name, ROUTER_FILTER_NAME);
        let hcm::http_connection_manager::RouteSpecifier::Rds(rds) =
            hcm.route_specifier.expect("route specifier");
        assert_eq!(rds.route_config_name, "payments:routes");
        assert!(rds.config_source.expect("config source").ads.is_some());
        assert_eq!(
            rds.config_source.unwrap().resource_api_version,
            hcm::ApiVersion::V3 as i32
        );
    }

    #[test]
    fn test_hcm_injects_rbac_filter_before_router() {
        use crate::xds::rbac::IntentionAction;
        use crate::xds::rbac::{
            HttpPermissionMatch, IntentionsForService, RbacIntention, RbacPermission,
            encode_rbac_filters,
        };

        let input = IntentionsForService {
            destination: "api".to_string(),
            trust_domain: "consul".to_string(),
            datacenter: "dc1".to_string(),
            intentions: vec![RbacIntention {
                source: "web".to_string(),
                precedence: 4,
                action: IntentionAction::Allow,
                permissions: vec![RbacPermission {
                    action: IntentionAction::Allow,
                    http: Some(HttpPermissionMatch {
                        path_prefix: Some("/admin".to_string()),
                        ..Default::default()
                    }),
                }],
            }],
        };
        let encoded = encode_rbac_filters(&input).expect("L7 intention must encode");
        let hcm_config = encode_http_connection_manager(
            "api:routes",
            HttpCodecType::Auto,
            "ingress_http",
            encoded.http_config.as_deref(),
        );
        let hcm = decode_http_connection_manager(&hcm_config).expect("HCM must decode");
        assert_eq!(hcm.http_filters.len(), 2);
        assert_eq!(hcm.http_filters[0].name, "envoy.filters.http.rbac");
        assert_eq!(hcm.http_filters[1].name, ROUTER_FILTER_NAME);
        let hcm::http_filter::ConfigType::TypedConfig(any) = hcm.http_filters[0]
            .config_type
            .as_ref()
            .expect("rbac typed_config");
        assert_eq!(any.type_url, type_urls::HTTP_RBAC_TYPE);
    }

    #[test]
    fn test_route_roundtrip_preserves_prefix_and_weighted_clusters() {
        let mut dest = RouteDestination::weighted(vec![
            WeightedCluster::new("payments-v1", 80),
            WeightedCluster::new("payments-v2", 20),
        ]);
        dest.timeout_ms = Some(3000);
        dest.prefix_rewrite = Some("/".to_string());

        let route = Route {
            name: "payments-route".to_string(),
            match_config: RouteMatch::prefix("/api")
                .with_header(HeaderMatcher::exact("x-env", "prod")),
            action: RouteAction::Route(dest),
            ..Default::default()
        };
        let config = RouteConfiguration {
            name: "payments:routes".to_string(),
            virtual_hosts: vec![VirtualHost {
                name: "payments".to_string(),
                domains: vec!["*".to_string()],
                routes: vec![route],
                ..Default::default()
            }],
            ..Default::default()
        };

        let bytes = encode_route(&config);
        let decoded = decode_route(&bytes).expect("must decode as RouteConfiguration");
        assert_eq!(decoded.name, "payments:routes");
        let vh = &decoded.virtual_hosts[0];
        assert_eq!(vh.domains, vec!["*".to_string()]);
        let r = &vh.routes[0];
        assert_eq!(r.name, "payments-route");
        assert_eq!(
            r.r#match.as_ref().unwrap().path_specifier,
            Some(rds::route_match::PathSpecifier::Prefix("/api".to_string()))
        );
        assert_eq!(r.r#match.as_ref().unwrap().headers[0].exact_match, "prod");
        let action = match r.action.as_ref().expect("action") {
            rds::route::Action::Route(a) => a,
            _ => panic!("expected route action"),
        };
        assert_eq!(action.prefix_rewrite, "/");
        assert_eq!(action.timeout.as_ref().unwrap().seconds, 3);
        let weighted = match action
            .cluster_specifier
            .as_ref()
            .expect("cluster specifier")
        {
            rds::route_action::ClusterSpecifier::WeightedClusters(w) => w,
            _ => panic!("expected weighted clusters"),
        };
        assert_eq!(weighted.clusters.len(), 2);
        assert_eq!(weighted.clusters[0].name, "payments-v1");
        assert_eq!(weighted.clusters[0].weight, Some(80));
        assert_eq!(weighted.total_weight, Some(100));
    }

    #[test]
    fn test_unsupported_matchers_and_filters_are_skipped_not_guessed() {
        // A listener whose only filter is an opaque JSON blob must be skipped.
        let opaque = NetworkFilter {
            name: "custom".to_string(),
            filter_type: NetworkFilterType::Generic {
                config: "{}".to_string(),
            },
        };
        let listener = Listener::new("opaque", ListenerAddress::tcp("0.0.0.0", 8080))
            .with_filter_chain(FilterChain::new("c").with_filter(opaque));
        assert!(encode_listener(&listener).is_none());

        // Unsupported path matchers must not be approximated by a prefix "/".
        let config = RouteConfiguration {
            name: "regex:routes".to_string(),
            virtual_hosts: vec![VirtualHost {
                name: "svc".to_string(),
                domains: vec!["*".to_string()],
                routes: vec![Route {
                    match_config: RouteMatch::regex("^/v[0-9]+/.*"),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        };
        let decoded = decode_route(&encode_route(&config)).unwrap();
        assert!(decoded.virtual_hosts[0].routes.is_empty());

        // A mismatched native variant must not be encoded as an SDS resource.
        assert!(
            encode_resource(ResourceType::Secret, NativeResource::Listener(&listener)).is_none()
        );
        // Runtime resources remain unsupported.
        assert!(
            encode_resource(ResourceType::Runtime, NativeResource::Listener(&listener)).is_none()
        );
    }

    #[test]
    fn test_json_is_not_a_valid_cluster_payload() {
        // Guards against regressing to the old `serde_json::to_vec` behaviour:
        // JSON bytes must not decode as an Envoy Cluster protobuf message.
        let cluster = Cluster::new_eds("payments");
        let json = serde_json::to_vec(&cluster).unwrap();
        assert!(cds::Cluster::decode(json.as_slice()).is_err());
    }

    #[test]
    fn test_secret_leaf_and_validation_roundtrip_with_exact_type_url() {
        use prost::Message;

        let leaf = SdsSecret::tls_certificate("server-cert", "CERT-PEM", "KEY-PEM");
        let ca = SdsSecret::validation_context("root-ca", "CA-PEM");

        for secret in [&leaf, &ca] {
            let bytes = encode_resource(ResourceType::Secret, NativeResource::Secret(secret))
                .expect("SDS must be encodable");
            let decoded = decode_secret(&bytes).expect("must decode as tls.Secret");
            assert_eq!(decoded.name, secret.name);
            assert!(decoded.r#type.is_some());

            // The payload must survive a google.protobuf.Any round trip with
            // the exact Envoy type URL (no short or legacy alias).
            let any = ::prost_types::Any {
                type_url: ResourceType::Secret.type_url().to_string(),
                value: bytes,
            };
            let any = ::prost_types::Any::decode(any.encode_to_vec().as_slice()).unwrap();
            assert_eq!(
                any.type_url,
                "type.googleapis.com/envoy.extensions.transport_sockets.tls.v3.Secret"
            );
            assert!(decode_secret(&any.value).is_ok());
        }

        match decode_secret(&encode_secret(&leaf))
            .unwrap()
            .r#type
            .unwrap()
        {
            tls::secret::Type::TlsCertificate(cert) => {
                assert_eq!(
                    cert.certificate_chain.unwrap().specifier,
                    Some(tls::data_source::Specifier::InlineString(
                        "CERT-PEM".to_string()
                    ))
                );
                assert_eq!(
                    cert.private_key.unwrap().specifier,
                    Some(tls::data_source::Specifier::InlineString(
                        "KEY-PEM".to_string()
                    ))
                );
            }
            _ => panic!("expected TLS certificate secret"),
        }
        match decode_secret(&encode_secret(&ca)).unwrap().r#type.unwrap() {
            tls::secret::Type::ValidationContext(ctx) => {
                assert_eq!(
                    ctx.trusted_ca.unwrap().specifier,
                    Some(tls::data_source::Specifier::InlineString(
                        "CA-PEM".to_string()
                    ))
                );
            }
            _ => panic!("expected validation context secret"),
        }
    }

    #[test]
    fn test_listener_filter_chain_carries_downstream_sds_transport_socket() {
        use prost::Message;

        let mut socket = NativeTransportSocket::downstream_sds("server-cert", "root-ca");
        if let TransportSocketConfig::DownstreamSds {
            require_client_certificate,
            alpn_protocols,
            ..
        } = &mut socket.config
        {
            *require_client_certificate = true;
            alpn_protocols.push("h2".to_string());
        }
        let listener = Listener::new("tls-listener", ListenerAddress::tcp("0.0.0.0", 8443))
            .with_filter_chain(
                FilterChain::new("tls")
                    .with_filter(NetworkFilter::tcp_proxy("payments"))
                    .with_transport_socket(socket),
            );

        let bytes = encode_listener(&listener).expect("listener must encode");
        let decoded = decode_listener(&bytes).unwrap();
        // Decoding into FilterChain.transport_socket proves the field-6 key.
        let transport = decoded.filter_chains[0]
            .transport_socket
            .as_ref()
            .expect("FilterChain.transport_socket (field 6)");
        assert_eq!(transport.name, "envoy.transport_sockets.tls");
        let any = transport.typed_config.as_ref().expect("typed_config");
        assert_eq!(any.type_url, type_urls::DOWNSTREAM_TLS_CONTEXT_TYPE);
        assert_eq!(
            any.type_url,
            "type.googleapis.com/envoy.extensions.transport_sockets.tls.v3.DownstreamTlsContext"
        );

        let ctx = tls::DownstreamTlsContext::decode(any.value.as_slice()).unwrap();
        assert_eq!(ctx.require_client_certificate, Some(true));
        let common = ctx.common_tls_context.unwrap();
        assert_eq!(common.alpn_protocols, vec!["h2".to_string()]);
        assert_eq!(
            common.tls_certificate_sds_secret_configs[0].name,
            "server-cert"
        );
        assert_eq!(
            common.validation_context_sds_secret_config.unwrap().name,
            "root-ca"
        );
        let source = common.tls_certificate_sds_secret_configs[0]
            .sds_config
            .as_ref()
            .unwrap();
        assert!(matches!(
            source.config_source_specifier,
            Some(tls::config_source::ConfigSourceSpecifier::Ads(_))
        ));
        assert_eq!(source.resource_api_version, tls::ApiVersion::V3 as i32);
    }

    #[test]
    fn test_cluster_carries_upstream_inline_transport_socket() {
        use prost::Message;

        let cluster = Cluster::new_static("payments").with_transport_socket(
            NativeTransportSocket::upstream_inline(
                "CERT",
                "KEY",
                "CA",
                "spiffe://cluster.local/ns/default/sa/app",
                "payments.default.svc",
            ),
        );
        let bytes = encode_cluster(&cluster);
        let decoded = decode_cluster(&bytes).unwrap();
        // Decoding into Cluster.transport_socket proves the field-24 key.
        let transport = decoded
            .transport_socket
            .expect("Cluster.transport_socket (field 24)");
        assert_eq!(transport.name, "envoy.transport_sockets.tls");
        let any = transport.typed_config.expect("typed_config");
        assert_eq!(any.type_url, type_urls::UPSTREAM_TLS_CONTEXT_TYPE);
        assert_eq!(
            any.type_url,
            "type.googleapis.com/envoy.extensions.transport_sockets.tls.v3.UpstreamTlsContext"
        );

        let ctx = tls::UpstreamTlsContext::decode(any.value.as_slice()).unwrap();
        assert_eq!(ctx.sni, "payments.default.svc");
        let common = ctx.common_tls_context.unwrap();
        assert_eq!(common.tls_certificates.len(), 1);
        assert_eq!(
            common.tls_certificates[0]
                .certificate_chain
                .as_ref()
                .unwrap()
                .specifier,
            Some(tls::data_source::Specifier::InlineString(
                "CERT".to_string()
            ))
        );
        let validation = common.validation_context.unwrap();
        assert_eq!(validation.match_typed_subject_alt_names.len(), 1);
        let matcher = &validation.match_typed_subject_alt_names[0];
        assert_eq!(matcher.san_type, tls::SanType::Uri as i32);
        let tls::string_matcher::MatchPattern::Exact(uri) = matcher
            .matcher
            .as_ref()
            .unwrap()
            .match_pattern
            .as_ref()
            .unwrap();
        assert_eq!(uri, "spiffe://cluster.local/ns/default/sa/app");
    }

    #[test]
    fn test_san_type_uri_wire_value_matches_envoy_v138() {
        // Envoy v1.38 SubjectAltNameMatcher.SanType: URI = 3, IP_ADDRESS = 4.
        // This is an enum number on the wire, so a local rename cannot hide a
        // mismatch: 4 would silently validate the wrong SAN class in Envoy.
        assert_eq!(tls::SanType::Uri as i32, 3);
        assert_eq!(tls::SanType::IpAddress as i32, 4);
    }
}
