//! Generated Envoy xDS v3 protocol types.
//!
//! These are produced by `build.rs` from `proto/*.proto` via `tonic-prost-build`
//! and included from `OUT_DIR`. They are the actual wire format spoken on the
//! ADS gRPC stream. Do not edit by hand; change the `.proto` files instead.
//!
//! Prost-generated items do not carry doc comments, so `missing_docs` (warned
//! at crate level) is scoped out here rather than annotating generated code.

#![allow(missing_docs)]
// Prost emits nested modules whose name matches their parent (e.g. `cluster`
// inside `envoy.config.cluster.v3`); that is normal generated code, not a
// hand-written design smell.
#![allow(clippy::all)]

/// ADS service and discovery messages (`envoy.service.discovery.v3`).
pub mod discovery {
    tonic::include_proto!("envoy.service.discovery.v3");
}

/// Cluster resources (`envoy.config.cluster.v3`).
pub mod cluster {
    tonic::include_proto!("envoy.config.cluster.v3");
}

/// Endpoint resources (`envoy.config.endpoint.v3`).
pub mod endpoint {
    tonic::include_proto!("envoy.config.endpoint.v3");
}

/// Listener resources (`envoy.config.listener.v3`).
pub mod listener {
    tonic::include_proto!("envoy.config.listener.v3");
}

/// Route resources (`envoy.config.route.v3`).
pub mod route {
    tonic::include_proto!("envoy.config.route.v3");
}

/// HTTP connection manager filter (`envoy.extensions.filters.network.http_connection_manager.v3`).
pub mod http_connection_manager {
    tonic::include_proto!("envoy.extensions.filters.network.http_connection_manager.v3");
}

/// TCP proxy filter (`envoy.extensions.filters.network.tcp_proxy.v3`).
pub mod tcp_proxy {
    tonic::include_proto!("envoy.extensions.filters.network.tcp_proxy.v3");
}

/// RBAC core config, matchers and filter typed_config wrappers
/// (`envoy.config.rbac.v3`). The network/HTTP filter configs share this
/// package so prost keeps them in one module; the encoder supplies the
/// correct upstream type_url for each filter.
pub mod rbac {
    tonic::include_proto!("envoy.config.rbac.v3");
}

/// Transport socket TLS configs and SDS secrets
/// (`envoy.extensions.transport_sockets.tls.v3`). The messages are a
/// self-contained, wire-compatible subset; the generated module is used both
/// for `Secret` resources and for the typed_config carried by transport
/// sockets.
pub mod tls {
    tonic::include_proto!("envoy.extensions.transport_sockets.tls.v3");
}

/// Istio MCP v1alpha1 stream messages (`istio.mcp.v1alpha1`).
/// Wire-compatible subset of the Nacos 3.3.0 MCP server protocol.
pub mod mcp {
    tonic::include_proto!("istio.mcp.v1alpha1");
}

/// Istio ServiceEntry wire model (`istio.networking.v1alpha3`).
/// Wire-compatible subset used as the Any body of MCP resources.
pub mod istio {
    tonic::include_proto!("istio.networking.v1alpha3");
}
