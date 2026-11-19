//! Runtime smoke test for the Consul Connect xDS data plane.
//!
//! Unlike the protocol-only tests in `batata-mesh/tests/ads_e2e.rs`, this test
//! drives the real `ConsulSyncBridge` five-second sync loop together with a
//! real tonic ADS server bound through `start_xds_grpc_server_on_listener`.
//! It reproduces the P0 regression: dropping either shutdown sender at startup
//! resolves its receiver, so the sync loop returns and the ADS server stops,
//! and no timed snapshot update can ever arrive.

use std::sync::Arc;
use std::time::Duration;

use batata_mesh::server::XdsServer;
use batata_mesh::xds::encode::{decode_cluster, decode_listener};
use batata_mesh::xds::generated::discovery::aggregated_discovery_service_client::AggregatedDiscoveryServiceClient;
use batata_mesh::xds::generated::discovery::{DiscoveryRequest, Node};
use batata_mesh::xds::type_urls;
use batata_plugin::PluginNamingStore;
use batata_plugin_consul::ConsulNamingStore;
use batata_plugin_consul::model::AgentServiceRegistration;
use batata_plugin_consul::namespace::DEFAULT_NAMESPACE;
use batata_plugin_consul::xds_sync::{ConsulSyncBridge, create_xds_server};
use tonic::transport::Channel;

fn request(type_url: &str) -> DiscoveryRequest {
    DiscoveryRequest {
        node: Some(Node {
            id: "runtime-smoke".to_string(),
            cluster: "web".to_string(),
            user_agent_name: "envoy".to_string(),
            ..Default::default()
        }),
        type_url: type_url.to_string(),
        ..Default::default()
    }
}

fn register(store: &ConsulNamingStore, service: &str, port: u16) {
    let id = format!("{}-1", service);
    let reg = AgentServiceRegistration {
        name: service.to_string(),
        id: Some(id.clone()),
        address: Some("127.0.0.1".to_string()),
        port: Some(port),
        ..Default::default()
    };
    let key = ConsulNamingStore::build_key(DEFAULT_NAMESPACE, service, &id);
    store
        .register(&key, serde_json::to_vec(&reg).unwrap().into())
        .unwrap();
}

#[tokio::test]
async fn test_connect_xds_runtime_stays_alive_and_pushes_timed_snapshots() {
    let naming_store = Arc::new(ConsulNamingStore::new());
    let xds_server: Arc<XdsServer> = create_xds_server();

    // Bind through the production entry point: a port conflict surfaces here
    // instead of being swallowed as a log line from a detached task.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let (grpc_shutdown_tx, grpc_shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let server_for_task = xds_server.clone();
    let server_task = tokio::spawn(async move {
        batata_mesh::start_xds_grpc_server_on_listener(server_for_task, listener, grpc_shutdown_rx)
            .await
            .expect("ADS server must run until shutdown");
    });

    // Wait until the listener accepts connections.
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

    register(&naming_store, "web", 8080);

    // Keep BOTH senders alive for the whole test, mirroring the plugin fix.
    let bridge = Arc::new(ConsulSyncBridge::new(
        naming_store.clone(),
        xds_server.clone(),
    ));
    let bridge_shutdown_tx = bridge.start();

    let mut client = AggregatedDiscoveryServiceClient::connect(format!("http://{addr}"))
        .await
        .unwrap();
    let (req_tx, req_rx) = tokio::sync::mpsc::channel::<DiscoveryRequest>(8);
    let mut stream = client
        .stream_aggregated_resources(tokio_stream::wrappers::ReceiverStream::new(req_rx))
        .await
        .unwrap()
        .into_inner();

    // The sync loop starts with an empty default snapshot, so the LDS request
    // has no immediate answer. The first 5-second tick must publish a snapshot
    // and push it on this stream, including an encodable Listener.
    req_tx
        .send(request(type_urls::LISTENER_TYPE))
        .await
        .unwrap();

    let mut lds_response = None;
    for _ in 0..8 {
        match tokio::time::timeout(Duration::from_secs(7), stream.message()).await {
            Ok(Ok(Some(resp)))
                if resp.type_url == type_urls::LISTENER_TYPE && !resp.resources.is_empty() =>
            {
                lds_response = Some(resp);
                break;
            }
            Ok(Ok(Some(_))) => continue,
            Ok(Ok(None)) | Ok(Err(_)) | Err(_) => break,
        }
    }
    let lds = lds_response.expect("timed LDS snapshot must be pushed after the 5s sync tick");
    let listener_msg = decode_listener(&lds.resources[0].value)
        .expect("pushed LDS Any.value must decode as an envoy Listener");
    assert!(!listener_msg.filter_chains.is_empty());

    // Register a second service and wait for the NEXT timed tick (no manual
    // `update_snapshot` call) to push a CDS response containing it.
    register(&naming_store, "api", 9090);
    let mut saw_api = false;
    for _ in 0..8 {
        match tokio::time::timeout(Duration::from_secs(7), stream.message()).await {
            Ok(Ok(Some(resp))) if resp.type_url == type_urls::CLUSTER_TYPE => {
                if resp.resources.iter().any(|any| {
                    decode_cluster(&any.value)
                        .map(|c| c.name == "api")
                        .unwrap_or(false)
                }) {
                    saw_api = true;
                    break;
                }
            }
            Ok(Ok(Some(_))) => continue,
            Ok(Ok(None)) | Ok(Err(_)) | Err(_) => break,
        }
    }
    assert!(
        saw_api,
        "second service must arrive through the timed sync loop, not a manual update"
    );

    // The server must still accept new streams well after startup. With the
    // shutdown senders dropped, tonic would already have stopped serving.
    let mut fresh_client = AggregatedDiscoveryServiceClient::connect(format!("http://{addr}"))
        .await
        .expect("ADS server must still be serving after two sync intervals");
    let outbound = tokio_stream::iter(vec![request(type_urls::LISTENER_TYPE)]);
    let mut fresh_stream = fresh_client
        .stream_aggregated_resources(outbound)
        .await
        .unwrap()
        .into_inner();
    let fresh = tokio::time::timeout(Duration::from_secs(5), fresh_stream.message())
        .await
        .unwrap()
        .unwrap()
        .expect("fresh stream must receive a response");
    assert_eq!(fresh.type_url, type_urls::LISTENER_TYPE);

    let _ = bridge_shutdown_tx.send(());
    let _ = grpc_shutdown_tx.send(());
    let _ = tokio::time::timeout(Duration::from_secs(5), server_task).await;
}
