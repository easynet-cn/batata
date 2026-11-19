//! End-to-end test for the Nacos-compatible Istio MCP `ResourceSource` stream.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use batata_mesh::conversion::{NacosInstance, NacosService};
use batata_mesh::xds::generated::cluster as cds_pb;
use batata_mesh::xds::generated::discovery::DiscoveryRequest;
use batata_mesh::xds::generated::discovery::aggregated_discovery_service_client::AggregatedDiscoveryServiceClient;
use batata_mesh::xds::generated::endpoint as eds_pb;
use batata_mesh::xds::generated::istio;
use batata_mesh::xds::generated::listener as lds_pb;
use batata_mesh::xds::generated::mcp::RequestResources;
use batata_mesh::xds::generated::mcp::Resource;
use batata_mesh::xds::generated::mcp::resource_source_client::ResourceSourceClient;
use batata_mesh::xds::generated::route as rds_pb;
use batata_mesh::{BatataMcpService, start_istio_mcp_grpc_server_on_listener};
use batata_mesh::{McpServer, McpServerConfig, SERVICE_ENTRY_COLLECTION};
use batata_mesh::{SyncBridgeConfig, build_resource_snapshot};
use prost::Message;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot};
use tokio::time::timeout;
use tokio_stream::wrappers::ReceiverStream;
use tonic::Request;

fn service_data() -> Vec<NacosService> {
    vec![NacosService {
        namespace_id: "public".to_string(),
        group_name: "DEFAULT_GROUP".to_string(),
        service_name: "web".to_string(),
        protect_threshold: 0.0,
        metadata: HashMap::new(),
        instances: vec![NacosInstance {
            instance_id: "web-1".to_string(),
            ip: "127.0.0.1".to_string(),
            port: 8080,
            weight: 1.0,
            healthy: true,
            enabled: true,
            ephemeral: true,
            cluster_name: String::new(),
            service_name: "web".to_string(),
            metadata: HashMap::new(),
        }],
    }]
}

#[tokio::test]
async fn mcp_stream_delivers_full_state_and_live_update() {
    let server = Arc::new(McpServer::new(McpServerConfig::default()));
    server.sync_services(&service_data()).await;
    let service = BatataMcpService::new(server.clone());

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let grpc_service = service.clone();
    let server_task = tokio::spawn(async move {
        start_istio_mcp_grpc_server_on_listener(grpc_service, listener, shutdown_rx)
            .await
            .expect("serve");
    });

    let (request_tx, request_rx) = mpsc::channel(4);
    request_tx
        .send(RequestResources {
            collection: SERVICE_ENTRY_COLLECTION.to_string(),
            ..Default::default()
        })
        .await
        .expect("send request");

    let mut client = timeout(Duration::from_secs(3), async {
        loop {
            match ResourceSourceClient::connect(format!("http://{addr}")).await {
                Ok(client) => return client,
                Err(_) => tokio::time::sleep(Duration::from_millis(20)).await,
            }
        }
    })
    .await
    .expect("connect");

    let mut stream = client
        .establish_resource_stream(Request::new(ReceiverStream::new(request_rx)))
        .await
        .expect("open stream")
        .into_inner();

    let first = timeout(Duration::from_secs(3), stream.message())
        .await
        .expect("first response")
        .expect("some response")
        .expect("ok response");
    assert_eq!(first.collection, "istio/networking/v1alpha3/serviceentries");
    assert!(!first.incremental);
    assert_eq!(first.resources.len(), 1);

    let body = first.resources[0].body.as_ref().expect("any body");
    assert_eq!(
        body.type_url,
        "type.googleapis.com/istio.networking.v1alpha3.ServiceEntry"
    );
    let service_entry = istio::ServiceEntry::decode(body.value.as_slice()).expect("decode");
    assert_eq!(service_entry.hosts, vec!["webnacos"]);
    assert_eq!(service_entry.ports[0].number, 0);
    assert_eq!(service_entry.endpoints[0].address, "127.0.0.1");

    let first_nonce = first.nonce.clone();
    request_tx
        .send(RequestResources {
            response_nonce: first_nonce.clone(),
            ..Default::default()
        })
        .await
        .expect("ack");

    server.sync_services(&[]).await;
    service.publish_snapshot();

    let second = timeout(Duration::from_secs(3), stream.message())
        .await
        .expect("second response")
        .expect("some response")
        .expect("ok response");
    assert_ne!(second.nonce, first_nonce);
    assert!(second.resources.is_empty());
    assert_eq!(second.collection, SERVICE_ENTRY_COLLECTION);

    drop(request_tx);
    let _ = shutdown_tx.send(());
    let _ = server_task.await;
}

#[tokio::test]
async fn ads_config_source_delivers_service_entry_and_live_update() {
    let server = Arc::new(McpServer::new(McpServerConfig::default()));
    server.sync_services(&service_data()).await;
    let service = BatataMcpService::new(server.clone());

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let grpc_service = service.clone();
    let server_task = tokio::spawn(async move {
        start_istio_mcp_grpc_server_on_listener(grpc_service, listener, shutdown_rx)
            .await
            .expect("serve");
    });

    let mut client = timeout(Duration::from_secs(3), async {
        loop {
            match AggregatedDiscoveryServiceClient::connect(format!("http://{addr}")).await {
                Ok(client) => return client,
                Err(_) => tokio::time::sleep(Duration::from_millis(20)).await,
            }
        }
    })
    .await
    .expect("connect");

    let (request_tx, request_rx) = mpsc::channel(4);
    request_tx
        .send(DiscoveryRequest {
            type_url: "networking.istio.io/v1alpha3/ServiceEntry".to_string(),
            ..Default::default()
        })
        .await
        .expect("send service entry request");
    request_tx
        .send(DiscoveryRequest {
            type_url: "core/v1alpha1/MeshConfig".to_string(),
            ..Default::default()
        })
        .await
        .expect("send mesh config request");

    let mut stream = client
        .stream_aggregated_resources(Request::new(ReceiverStream::new(request_rx)))
        .await
        .expect("open ADS stream")
        .into_inner();

    let first = timeout(Duration::from_secs(3), stream.message())
        .await
        .expect("first ADS response")
        .expect("some ADS response")
        .expect("ok ADS response");
    assert_eq!(first.version_info, "1");
    assert_eq!(first.type_url, "networking.istio.io/v1alpha3/ServiceEntry");
    assert_eq!(first.resources.len(), 1);
    let outer = &first.resources[0];
    assert_eq!(
        outer.type_url,
        "type.googleapis.com/istio.mcp.v1alpha1.Resource"
    );
    let resource = Resource::decode(outer.value.as_slice()).expect("decode MCP resource");
    let body = resource.body.expect("resource body");
    assert_eq!(
        body.type_url,
        "type.googleapis.com/istio.networking.v1alpha3.ServiceEntry"
    );
    let service_entry = istio::ServiceEntry::decode(body.value.as_slice()).expect("decode SE");
    assert_eq!(service_entry.hosts, vec!["webnacos"]);

    let first_nonce = first.nonce.clone();
    request_tx
        .send(DiscoveryRequest {
            type_url: "networking.istio.io/v1alpha3/ServiceEntry".to_string(),
            response_nonce: first_nonce.clone(),
            ..Default::default()
        })
        .await
        .expect("ack");

    server.sync_services(&[]).await;
    service.publish_snapshot();

    let second = timeout(Duration::from_secs(3), stream.message())
        .await
        .expect("second ADS response")
        .expect("some ADS response")
        .expect("ok ADS response");
    assert_ne!(second.nonce, first_nonce);
    assert_eq!(second.type_url, "networking.istio.io/v1alpha3/ServiceEntry");
    assert!(second.resources.is_empty());

    drop(request_tx);
    let _ = shutdown_tx.send(());
    let _ = server_task.await;
}

#[tokio::test]
async fn ads_config_source_delivers_envoy_xds_resources() {
    let server = Arc::new(McpServer::new(McpServerConfig::default()));
    server.sync_services(&service_data()).await;
    let service = BatataMcpService::new(server.clone());
    service.publish_snapshot_with_xds(build_resource_snapshot(
        &service_data(),
        &SyncBridgeConfig::default(),
    ));
    let xds_version = service.current_xds_snapshot().version.clone();

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let grpc_service = service.clone();
    let server_task = tokio::spawn(async move {
        start_istio_mcp_grpc_server_on_listener(grpc_service, listener, shutdown_rx)
            .await
            .expect("serve");
    });

    let mut client = timeout(Duration::from_secs(3), async {
        loop {
            match AggregatedDiscoveryServiceClient::connect(format!("http://{addr}")).await {
                Ok(client) => return client,
                Err(_) => tokio::time::sleep(Duration::from_millis(20)).await,
            }
        }
    })
    .await
    .expect("connect");

    let (request_tx, request_rx) = mpsc::channel(8);
    for type_url in [
        "type.googleapis.com/envoy.config.cluster.v3.Cluster",
        "type.googleapis.com/envoy.config.endpoint.v3.ClusterLoadAssignment",
        "type.googleapis.com/envoy.config.listener.v3.Listener",
        "type.googleapis.com/envoy.config.route.v3.RouteConfiguration",
    ] {
        request_tx
            .send(DiscoveryRequest {
                type_url: type_url.to_string(),
                ..Default::default()
            })
            .await
            .expect("send xDS request");
    }

    let mut stream = client
        .stream_aggregated_resources(Request::new(ReceiverStream::new(request_rx)))
        .await
        .expect("open ADS stream")
        .into_inner();

    let mut responses = Vec::new();
    for _ in 0..4 {
        responses.push(
            timeout(Duration::from_secs(3), stream.message())
                .await
                .expect("xDS response")
                .expect("some response")
                .expect("ok response"),
        );
    }

    let cds = &responses[0];
    assert_eq!(
        cds.type_url,
        "type.googleapis.com/envoy.config.cluster.v3.Cluster"
    );
    assert_eq!(cds.version_info, xds_version);
    assert_eq!(cds.resources.len(), 1);
    let any = &cds.resources[0];
    assert_eq!(
        any.type_url,
        "type.googleapis.com/envoy.config.cluster.v3.Cluster"
    );
    let cluster = cds_pb::Cluster::decode(any.value.as_slice()).expect("decode CDS");
    assert_eq!(cluster.name, "web");

    let eds = &responses[1];
    assert_eq!(
        eds.type_url,
        "type.googleapis.com/envoy.config.endpoint.v3.ClusterLoadAssignment"
    );
    assert_eq!(eds.version_info, xds_version);
    let any = &eds.resources[0];
    let cla = eds_pb::ClusterLoadAssignment::decode(any.value.as_slice()).expect("decode EDS");
    assert_eq!(cla.cluster_name, "web");

    let lds = &responses[2];
    assert_eq!(
        lds.type_url,
        "type.googleapis.com/envoy.config.listener.v3.Listener"
    );
    assert_eq!(lds.version_info, xds_version);
    let any = &lds.resources[0];
    let listener = lds_pb::Listener::decode(any.value.as_slice()).expect("decode LDS");
    assert_eq!(listener.name, "ingress-listener");

    let rds = &responses[3];
    assert_eq!(
        rds.type_url,
        "type.googleapis.com/envoy.config.route.v3.RouteConfiguration"
    );
    assert_eq!(rds.version_info, xds_version);
    let any = &rds.resources[0];
    let config = rds_pb::RouteConfiguration::decode(any.value.as_slice()).expect("decode RDS");
    assert_eq!(config.name, "default-routes");

    drop(request_tx);
    let _ = shutdown_tx.send(());
    let _ = server_task.await;
}

#[tokio::test]
async fn ads_ack_with_new_resource_names_answers_new_subscription() {
    let mut services = service_data();
    let mut api = services[0].clone();
    api.service_name = "api".to_string();
    api.instances[0].instance_id = "api-1".to_string();
    api.instances[0].service_name = "api".to_string();
    api.instances[0].port = 8081;
    services.push(api);

    let server = Arc::new(McpServer::new(McpServerConfig::default()));
    let service = BatataMcpService::new(server.clone());
    service.publish_snapshot_with_xds(build_resource_snapshot(
        &services,
        &SyncBridgeConfig::default(),
    ));

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let grpc_service = service.clone();
    let server_task = tokio::spawn(async move {
        start_istio_mcp_grpc_server_on_listener(grpc_service, listener, shutdown_rx)
            .await
            .expect("serve");
    });

    let mut client = timeout(Duration::from_secs(3), async {
        loop {
            match AggregatedDiscoveryServiceClient::connect(format!("http://{addr}")).await {
                Ok(client) => return client,
                Err(_) => tokio::time::sleep(Duration::from_millis(20)).await,
            }
        }
    })
    .await
    .expect("connect");

    let (request_tx, request_rx) = mpsc::channel(4);
    let eds_type = "type.googleapis.com/envoy.config.endpoint.v3.ClusterLoadAssignment";
    request_tx
        .send(DiscoveryRequest {
            type_url: eds_type.to_string(),
            resource_names: vec!["web".to_string()],
            ..Default::default()
        })
        .await
        .expect("send initial EDS subscription");

    let mut stream = client
        .stream_aggregated_resources(Request::new(ReceiverStream::new(request_rx)))
        .await
        .expect("open ADS stream")
        .into_inner();

    let first = timeout(Duration::from_secs(3), stream.message())
        .await
        .expect("first EDS response")
        .expect("some response")
        .expect("ok response");
    assert_eq!(first.resources.len(), 1);
    let cla = eds_pb::ClusterLoadAssignment::decode(first.resources[0].value.as_slice())
        .expect("decode first EDS");
    assert_eq!(cla.cluster_name, "web");

    request_tx
        .send(DiscoveryRequest {
            type_url: eds_type.to_string(),
            resource_names: vec!["web".to_string()],
            response_nonce: first.nonce.clone(),
            ..Default::default()
        })
        .await
        .expect("ack first EDS");
    request_tx
        .send(DiscoveryRequest {
            type_url: eds_type.to_string(),
            resource_names: vec!["api".to_string()],
            response_nonce: first.nonce.clone(),
            ..Default::default()
        })
        .await
        .expect("resubscribe EDS to api");

    let second = timeout(Duration::from_secs(3), stream.message())
        .await
        .expect("second EDS response")
        .expect("some response")
        .expect("ok response");
    assert_ne!(second.nonce, first.nonce);
    assert_eq!(second.resources.len(), 1);
    let cla = eds_pb::ClusterLoadAssignment::decode(second.resources[0].value.as_slice())
        .expect("decode second EDS");
    assert_eq!(cla.cluster_name, "api");

    drop(request_tx);
    let _ = shutdown_tx.send(());
    let _ = server_task.await;
}
