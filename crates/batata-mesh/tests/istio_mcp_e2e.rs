//! End-to-end test for the Nacos-compatible Istio MCP `ResourceSource` stream.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use batata_mesh::conversion::{NacosInstance, NacosService};
use batata_mesh::xds::generated::istio;
use batata_mesh::xds::generated::mcp::RequestResources;
use batata_mesh::xds::generated::mcp::resource_source_client::ResourceSourceClient;
use batata_mesh::{BatataMcpService, start_istio_mcp_grpc_server_on_listener};
use batata_mesh::{McpServer, McpServerConfig, SERVICE_ENTRY_COLLECTION};
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
