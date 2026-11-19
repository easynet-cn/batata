//! End-to-end verification of the Envoy ADS data plane.
//!
//! Spins up the real tonic-based ADS server, connects with the generated ADS
//! client, sends CDS/EDS/LDS/RDS discovery requests, and decodes `Any.value`
//! back into the generated Envoy protobuf messages. This is the check that a
//! real Envoy sidecar would perform when unpacking a discovery response.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use batata_mesh::server::{XdsServer, XdsServerConfig};
use batata_mesh::snapshot::ResourceSnapshot;
use batata_mesh::xds::ResourceType;
use batata_mesh::xds::encode::{
    decode_cluster, decode_endpoints, decode_http_connection_manager, decode_listener,
    decode_route, decode_tcp_proxy,
};
use batata_mesh::xds::generated::discovery::aggregated_discovery_service_client::AggregatedDiscoveryServiceClient;
use batata_mesh::xds::generated::discovery::{DiscoveryRequest, Node};
use batata_mesh::xds::generated::{
    cluster as cds, http_connection_manager as hcm, listener as lds, route as rds,
};
use batata_mesh::xds::type_urls;
use batata_mesh::xds::types::{
    Cluster, ClusterLoadAssignment, Endpoint, FilterChain, Listener, ListenerAddress, Locality,
    NetworkFilter, Route, RouteAction, RouteConfiguration, RouteDestination, RouteMatch,
    VirtualHost,
};
use tonic::transport::Channel;
use tonic::transport::Server;

fn sample_snapshot() -> ResourceSnapshot {
    let mut cluster = Cluster::new_eds("payments");
    cluster.connect_timeout_ms = 3_000;

    let mut cla = ClusterLoadAssignment::new("payments");
    cla.add_locality(
        Locality::new("region-a", "zone-1"),
        vec![
            Endpoint::new(SocketAddr::new(
                IpAddr::V4(Ipv4Addr::new(10, 1, 2, 3)),
                9_001,
            )),
            Endpoint::new(SocketAddr::new(
                IpAddr::V4(Ipv4Addr::new(10, 1, 2, 4)),
                9_002,
            )),
        ],
        200,
    );

    let inbound = Listener::new(
        "inbound_listener:21000",
        ListenerAddress::tcp("0.0.0.0", 21000),
    )
    .with_filter_chain(
        FilterChain::new("inbound").with_filter(NetworkFilter::tcp_proxy("payments")),
    );
    let outbound = Listener::new(
        "outbound_listener:9090",
        ListenerAddress::tcp("127.0.0.1", 9090),
    )
    .with_filter_chain(
        FilterChain::new("http")
            .with_filter(NetworkFilter::http_connection_manager("payments:routes")),
    );

    let route_config = RouteConfiguration::new("payments:routes").with_virtual_host(
        VirtualHost::new("payments", vec!["*".to_string()]).with_route(Route::new(
            "payments-api",
            RouteMatch::prefix("/api"),
            RouteAction::Route(RouteDestination::cluster("payments")),
        )),
    );

    ResourceSnapshot::with_all_resources(
        vec![cluster],
        vec![cla],
        vec![inbound, outbound],
        vec![route_config],
    )
}

async fn start_server() -> (SocketAddr, Arc<XdsServer>, tokio::sync::oneshot::Sender<()>) {
    let xds_server = Arc::new(XdsServer::new(XdsServerConfig::default()));
    xds_server.update_snapshot(sample_snapshot());

    // Bind port 0 to let the OS pick a free port, avoiding interference with
    // other test suites / local services.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);

    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let svc = batata_mesh::ads::BatataAdsService::new(xds_server.clone());
    tokio::spawn(async move {
        Server::builder()
            .add_service(svc.into_server())
            .serve_with_shutdown(addr, async {
                let _ = rx.await;
            })
            .await
            .unwrap();
    });

    // Wait until the server accepts connections.
    for _ in 0..50 {
        if Channel::from_shared(format!("http://{addr}"))
            .unwrap()
            .connect()
            .await
            .is_ok()
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    (addr, xds_server, tx)
}

fn request(type_url: &str) -> DiscoveryRequest {
    DiscoveryRequest {
        node: Some(Node {
            id: "sidecar-e2e".to_string(),
            cluster: "payments".to_string(),
            user_agent_name: "envoy".to_string(),
            ..Default::default()
        }),
        type_url: type_url.to_string(),
        ..Default::default()
    }
}

#[tokio::test]
async fn test_ads_cds_response_is_decodable_envoy_protobuf() {
    let (addr, _server, shutdown) = start_server().await;
    let mut client = AggregatedDiscoveryServiceClient::connect(format!("http://{addr}"))
        .await
        .unwrap();

    let outbound = tokio_stream::iter(vec![request(ResourceType::Cluster.type_url())]);
    let mut stream = client
        .stream_aggregated_resources(outbound)
        .await
        .unwrap()
        .into_inner();

    let response = tokio::time::timeout(Duration::from_secs(5), stream.message())
        .await
        .expect("ADS server must respond within timeout")
        .unwrap()
        .expect("ADS response must be present");

    assert_eq!(response.type_url, ResourceType::Cluster.type_url());
    assert!(!response.version_info.is_empty());
    assert!(!response.nonce.is_empty());
    assert_eq!(response.resources.len(), 1);

    let any = &response.resources[0];
    assert_eq!(any.type_url, ResourceType::Cluster.type_url());
    let cluster = decode_cluster(&any.value).expect("Any.value must decode as envoy Cluster");
    assert_eq!(cluster.name, "payments");
    assert_eq!(cluster.r#type, cds::cluster::DiscoveryType::Eds as i32);
    assert_eq!(cluster.connect_timeout.unwrap().seconds, 3);
    assert!(
        cluster
            .eds_cluster_config
            .unwrap()
            .eds_config
            .unwrap()
            .ads
            .is_some()
    );

    let _ = shutdown.send(());
}

#[tokio::test]
async fn test_ads_eds_response_is_decodable_envoy_protobuf() {
    let (addr, _server, shutdown) = start_server().await;
    let mut client = AggregatedDiscoveryServiceClient::connect(format!("http://{addr}"))
        .await
        .unwrap();

    let outbound = tokio_stream::iter(vec![request(ResourceType::Endpoint.type_url())]);
    let mut stream = client
        .stream_aggregated_resources(outbound)
        .await
        .unwrap()
        .into_inner();

    let response = tokio::time::timeout(Duration::from_secs(5), stream.message())
        .await
        .expect("ADS server must respond within timeout")
        .unwrap()
        .expect("ADS response must be present");

    assert_eq!(response.type_url, ResourceType::Endpoint.type_url());
    assert_eq!(response.resources.len(), 1);
    let any = &response.resources[0];
    let cla = decode_endpoints(&any.value).expect("Any.value must decode as envoy CLA");
    assert_eq!(cla.cluster_name, "payments");
    let group = &cla.endpoints[0];
    assert_eq!(group.lb_endpoints.len(), 2);
    assert_eq!(group.locality.as_ref().unwrap().region, "region-a");
    assert_eq!(group.load_balancing_weight, Some(200));

    let _ = shutdown.send(());
}

#[tokio::test]
async fn test_ads_lds_response_carries_decodable_filter_configs() {
    let (addr, _server, shutdown) = start_server().await;
    let mut client = AggregatedDiscoveryServiceClient::connect(format!("http://{addr}"))
        .await
        .unwrap();

    let outbound = tokio_stream::iter(vec![request(ResourceType::Listener.type_url())]);
    let mut stream = client
        .stream_aggregated_resources(outbound)
        .await
        .unwrap()
        .into_inner();

    let response = tokio::time::timeout(Duration::from_secs(5), stream.message())
        .await
        .expect("ADS server must respond within timeout")
        .unwrap()
        .expect("ADS response must be present");

    assert_eq!(response.type_url, ResourceType::Listener.type_url());
    assert_eq!(response.resources.len(), 2);

    let mut saw_tcp_proxy = false;
    let mut saw_hcm_rds = false;
    for any in &response.resources {
        assert_eq!(any.type_url, ResourceType::Listener.type_url());
        let listener = decode_listener(&any.value).expect("must decode as envoy Listener");
        let address = listener.address.expect("address").address.expect("socket");
        match address {
            lds::address::Address::SocketAddress(sa) => {
                assert!(sa.port_specifier.is_some());
            }
        }
        for chain in &listener.filter_chains {
            for filter in &chain.filters {
                let lds::filter::ConfigType::TypedConfig(config) =
                    filter.config_type.as_ref().expect("typed_config");
                if config.type_url == type_urls::TCP_PROXY_TYPE {
                    saw_tcp_proxy = true;
                    let tcp = decode_tcp_proxy(&config.value).expect("must decode TcpProxy");
                    assert_eq!(tcp.cluster, "payments");
                } else if config.type_url == type_urls::HCM_TYPE {
                    saw_hcm_rds = true;
                    let manager = decode_http_connection_manager(&config.value)
                        .expect("must decode HttpConnectionManager");
                    assert_eq!(manager.http_filters.len(), 1);
                    let hcm::http_connection_manager::RouteSpecifier::Rds(route_source) =
                        manager.route_specifier.expect("route specifier");
                    assert_eq!(route_source.route_config_name, "payments:routes");
                    assert!(
                        route_source
                            .config_source
                            .expect("config source")
                            .ads
                            .is_some()
                    );
                } else {
                    panic!("unexpected filter type_url: {}", config.type_url);
                }
            }
        }
    }
    assert!(saw_tcp_proxy, "TCP proxy listener must be served over LDS");
    assert!(saw_hcm_rds, "HCM/RDS listener must be served over LDS");

    let _ = shutdown.send(());
}

#[tokio::test]
async fn test_ads_rds_response_is_decodable_envoy_protobuf() {
    let (addr, _server, shutdown) = start_server().await;
    let mut client = AggregatedDiscoveryServiceClient::connect(format!("http://{addr}"))
        .await
        .unwrap();

    let outbound = tokio_stream::iter(vec![request(ResourceType::Route.type_url())]);
    let mut stream = client
        .stream_aggregated_resources(outbound)
        .await
        .unwrap()
        .into_inner();

    let response = tokio::time::timeout(Duration::from_secs(5), stream.message())
        .await
        .expect("ADS server must respond within timeout")
        .unwrap()
        .expect("ADS response must be present");

    assert_eq!(response.type_url, ResourceType::Route.type_url());
    assert_eq!(response.resources.len(), 1);
    let any = &response.resources[0];
    assert_eq!(any.type_url, ResourceType::Route.type_url());
    let config = decode_route(&any.value).expect("Any.value must decode as RouteConfiguration");
    assert_eq!(config.name, "payments:routes");
    assert_eq!(config.virtual_hosts.len(), 1);
    assert_eq!(config.virtual_hosts[0].domains, vec!["*".to_string()]);
    let route = &config.virtual_hosts[0].routes[0];
    assert_eq!(route.name, "payments-api");
    assert_eq!(
        route.r#match.as_ref().unwrap().path_specifier,
        Some(rds::route_match::PathSpecifier::Prefix("/api".to_string()))
    );
    let action = match route.action.as_ref().expect("route action") {
        rds::route::Action::Route(a) => a,
        _ => panic!("expected a cluster route action"),
    };
    assert_eq!(
        action.cluster_specifier,
        Some(rds::route_action::ClusterSpecifier::Cluster(
            "payments".to_string()
        ))
    );

    let _ = shutdown.send(());
}

#[tokio::test]
async fn test_ads_pushes_snapshot_updates_without_new_request() {
    let (addr, server, shutdown) = start_server().await;
    let mut client = AggregatedDiscoveryServiceClient::connect(format!("http://{addr}"))
        .await
        .unwrap();

    let (req_tx, req_rx) = tokio::sync::mpsc::channel::<DiscoveryRequest>(4);
    let mut stream = client
        .stream_aggregated_resources(tokio_stream::wrappers::ReceiverStream::new(req_rx))
        .await
        .unwrap()
        .into_inner();

    // Initial CDS request registers the stream for server-side pushes.
    req_tx
        .send(request(ResourceType::Cluster.type_url()))
        .await
        .unwrap();
    let first = tokio::time::timeout(Duration::from_secs(5), stream.message())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(first.type_url, ResourceType::Cluster.type_url());

    // Updating the snapshot must push a new response on the same stream.
    server.update_snapshot(sample_snapshot());
    let pushed = tokio::time::timeout(Duration::from_secs(5), stream.message())
        .await
        .expect("snapshot update must be pushed")
        .unwrap()
        .expect("push response must be present");

    assert_eq!(pushed.type_url, ResourceType::Cluster.type_url());
    assert!(!pushed.resources.is_empty());
    let cluster = decode_cluster(&pushed.resources[0].value).unwrap();
    assert_eq!(cluster.name, "payments");

    let _ = shutdown.send(());
}

#[tokio::test]
async fn test_ads_snapshot_push_includes_lds_and_rds() {
    let (addr, server, shutdown) = start_server().await;
    let mut client = AggregatedDiscoveryServiceClient::connect(format!("http://{addr}"))
        .await
        .unwrap();

    let (req_tx, req_rx) = tokio::sync::mpsc::channel::<DiscoveryRequest>(4);
    let mut stream = client
        .stream_aggregated_resources(tokio_stream::wrappers::ReceiverStream::new(req_rx))
        .await
        .unwrap()
        .into_inner();

    req_tx
        .send(request(ResourceType::Cluster.type_url()))
        .await
        .unwrap();
    let _ = tokio::time::timeout(Duration::from_secs(5), stream.message())
        .await
        .unwrap()
        .unwrap()
        .unwrap();

    server.update_snapshot(sample_snapshot());

    // One update pushes CDS + EDS + LDS + RDS on the same stream.
    let mut seen = std::collections::HashSet::new();
    for _ in 0..4 {
        let response = tokio::time::timeout(Duration::from_secs(5), stream.message())
            .await
            .expect("snapshot update must push all four resource types")
            .unwrap()
            .expect("push response must be present");
        seen.insert(response.type_url.clone());
    }
    assert!(seen.contains(ResourceType::Cluster.type_url()));
    assert!(seen.contains(ResourceType::Endpoint.type_url()));
    assert!(seen.contains(ResourceType::Listener.type_url()));
    assert!(seen.contains(ResourceType::Route.type_url()));

    let _ = shutdown.send(());
}
