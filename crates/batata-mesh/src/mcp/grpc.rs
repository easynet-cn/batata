//! Tonic `ResourceSource` service for the Nacos-compatible Istio MCP port.
//!
//! Upstream Nacos serves full-state MCP pushes on a long-lived bidirectional
//! stream. Batata mirrors that behavior: the first `RequestResources` selects
//! the collection and receives the current snapshot; later Naming changes are
//! pushed as complete replacements with a fresh nonce. ACK/NACK requests are
//! logged and, like upstream, do not themselves trigger a push.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio_stream::wrappers::{ReceiverStream, TcpListenerStream};
use tonic::{Request, Response, Status, Streaming};
use tracing::{debug, info, warn};

use crate::snapshot::ResourceSnapshot;
use crate::xds::generated::mcp::resource_source_server::ResourceSource;
use crate::xds::generated::mcp::{RequestResources, Resources};

use super::encode::{SERVICE_ENTRY_COLLECTION, encode_resources};
use super::server::McpServer;
use super::types::ServiceEntry;

/// Snapshot published to all connected MCP and config-source ADS sinks.
#[derive(Clone, Debug)]
pub struct McpSnapshot {
    /// Monotonic native version from [`McpServer`].
    pub version: u64,
    /// Complete ServiceEntry collection.
    pub entries: Vec<ServiceEntry>,
    /// Nonce paired with this response.
    pub nonce: String,
    /// Naming-derived Envoy resources served through the config-source ADS
    /// stream on the same port. Kept separate from the MCP version because
    /// upstream uses one resource-snapshot version for CDS/EDS/LDS/RDS.
    pub xds: Arc<ResourceSnapshot>,
}

impl Default for McpSnapshot {
    fn default() -> Self {
        Self {
            version: 0,
            entries: Vec::new(),
            nonce: String::new(),
            xds: Arc::new(ResourceSnapshot::new()),
        }
    }
}

struct McpInner {
    server: Arc<McpServer>,
    snapshot_tx: broadcast::Sender<Arc<McpSnapshot>>,
    xds_snapshot: parking_lot::RwLock<Arc<ResourceSnapshot>>,
    nonce_counter: AtomicU64,
}

/// MCP resource source shared by all connected streams.
#[derive(Clone)]
pub struct BatataMcpService {
    inner: Arc<McpInner>,
}

impl BatataMcpService {
    /// Create a resource source over an existing native MCP server.
    pub fn new(server: Arc<McpServer>) -> Self {
        let (snapshot_tx, _) = broadcast::channel(64);
        Self {
            inner: Arc::new(McpInner {
                server,
                snapshot_tx,
                xds_snapshot: parking_lot::RwLock::new(Arc::new(ResourceSnapshot::new())),
                nonce_counter: AtomicU64::new(0),
            }),
        }
    }

    /// Get the native MCP server backing this resource source.
    pub fn server(&self) -> Arc<McpServer> {
        self.inner.server.clone()
    }

    /// Subscribe to published full-state snapshots.
    pub fn subscribe(&self) -> broadcast::Receiver<Arc<McpSnapshot>> {
        self.inner.snapshot_tx.subscribe()
    }

    /// Publish the current full state to all connected streams.
    ///
    /// The naming adapter calls this once after each successful
    /// [`McpServer::sync_services`] so sinks receive one response per change
    /// batch rather than one response per generated resource.
    pub fn publish_snapshot(&self) {
        let snapshot = Arc::new(self.current_snapshot());
        if self.inner.snapshot_tx.send(snapshot).is_err() {
            debug!("no MCP sink connected; snapshot not broadcast");
        }
    }

    /// Replace the Envoy xDS snapshot and publish one combined full-state
    /// update. Updating and broadcasting under one call avoids an ADS stream
    /// observing ServiceEntry state from one naming batch with CDS/EDS/LDS/RDS
    /// state from another.
    pub fn publish_snapshot_with_xds(&self, xds: ResourceSnapshot) {
        *self.inner.xds_snapshot.write() = Arc::new(xds);
        self.publish_snapshot();
    }

    /// Current naming-derived Envoy xDS snapshot used by config-source ADS.
    pub fn current_xds_snapshot(&self) -> Arc<ResourceSnapshot> {
        self.inner.xds_snapshot.read().clone()
    }

    /// Build a snapshot from the current native cache and a fresh nonce.
    pub(crate) fn current_snapshot(&self) -> McpSnapshot {
        McpSnapshot {
            version: self.inner.server.version(),
            entries: self.inner.server.get_service_entries(),
            nonce: self.next_nonce(),
            xds: self.current_xds_snapshot(),
        }
    }

    /// Generate a process-local nonce. Upstream uses a dashed UUID; the wire
    /// only requires an opaque unique token, so timestamp + counter is
    /// sufficient and avoids pulling in another dependency.
    pub(crate) fn next_nonce(&self) -> String {
        let counter = self.inner.nonce_counter.fetch_add(1, Ordering::SeqCst);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        format!("{:016x}{:016x}", now, counter)
    }

    fn response(&self, collection: &str, snapshot: &McpSnapshot) -> Resources {
        if collection == SERVICE_ENTRY_COLLECTION {
            encode_resources(&snapshot.entries, snapshot.version, snapshot.nonce.clone())
        } else {
            // Upstream routes unknown collections to an empty generator.
            Resources {
                system_version_info: snapshot.version.to_string(),
                collection: collection.to_string(),
                resources: Vec::new(),
                removed_resources: Vec::new(),
                nonce: snapshot.nonce.clone(),
                incremental: false,
            }
        }
    }
}

#[tonic::async_trait]
impl ResourceSource for BatataMcpService {
    type EstablishResourceStreamStream = ReceiverStream<Result<Resources, Status>>;

    async fn establish_resource_stream(
        &self,
        request: Request<Streaming<RequestResources>>,
    ) -> Result<Response<Self::EstablishResourceStreamStream>, Status> {
        let mut inbound = request.into_inner();
        let mut updates = self.inner.snapshot_tx.subscribe();
        let (tx, rx) = mpsc::channel(16);
        let service = self.clone();

        tokio::spawn(async move {
            let Some(first) = inbound.message().await.ok().flatten() else {
                debug!("MCP stream closed before first request");
                return;
            };

            if first.incremental {
                // Upstream logs incremental requests but still answers with
                // full state; keep that compatibility behavior.
                debug!("MCP incremental requested; sending full state");
            }

            let collection = first.collection;
            let snapshot = service.current_snapshot();
            if tx
                .send(Ok(service.response(&collection, &snapshot)))
                .await
                .is_err()
            {
                return;
            }

            loop {
                tokio::select! {
                    incoming = inbound.message() => {
                        match incoming {
                            Ok(Some(req)) => {
                                if let Some(status) = req.error_detail {
                                    if status.code != 0 {
                                        warn!(
                                            code = status.code,
                                            message = %status.message,
                                            nonce = %req.response_nonce,
                                            "MCP sink NACK"
                                        );
                                    } else {
                                        debug!(nonce = %req.response_nonce, "MCP sink ACK");
                                    }
                                }
                            }
                            Ok(None) => break,
                            Err(error) => {
                                warn!(%error, "MCP stream receive failed");
                                break;
                            }
                        }
                    }
                    update = updates.recv() => {
                        let snapshot = match update {
                            Ok(snapshot) => snapshot,
                            Err(broadcast::error::RecvError::Lagged(skipped)) => {
                                warn!(skipped, "MCP sink lagged; resending current state");
                                Arc::new(service.current_snapshot())
                            }
                            Err(broadcast::error::RecvError::Closed) => break,
                        };
                        if tx
                            .send(Ok(service.response(&collection, &snapshot)))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

/// Serve MCP on an already-bound listener.
///
/// Binding belongs to the caller so port conflicts fail startup synchronously.
pub async fn start_istio_mcp_grpc_server_on_listener(
    service: BatataMcpService,
    listener: TcpListener,
    shutdown_rx: oneshot::Receiver<()>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use tonic::transport::Server;

    let addr = listener.local_addr()?;
    info!(addr = %addr, "Starting Istio MCP gRPC server");

    let mcp_service = crate::xds::generated::mcp::resource_source_server::ResourceSourceServer::new(
        service.clone(),
    );
    let ads_service = super::ads::BatataIstioMcpAdsService::new(service).into_server();
    Server::builder()
        .add_service(mcp_service)
        .add_service(ads_service)
        .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async {
            let _ = shutdown_rx.await;
            info!("Istio MCP gRPC server shutdown signal received");
        })
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonce_is_unique_and_opaque() {
        let service = BatataMcpService::new(Arc::new(McpServer::new(Default::default())));
        let first = service.next_nonce();
        let second = service.next_nonce();

        assert_ne!(first, second);
        assert_eq!(first.len(), 32);
        assert!(first.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn unknown_collection_gets_empty_full_state_response() {
        let service = BatataMcpService::new(Arc::new(McpServer::new(Default::default())));
        let snapshot = service.current_snapshot();

        let response = service.response("istio/networking/v1alpha3/virtualservices", &snapshot);

        assert_eq!(
            response.collection,
            "istio/networking/v1alpha3/virtualservices"
        );
        assert!(response.resources.is_empty());
        assert!(!response.incremental);
        assert_eq!(response.nonce, snapshot.nonce);
    }
}
