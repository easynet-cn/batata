//! Consul Peering API
//!
//! Provides cluster peering endpoints for cross-datacenter service discovery.

use actix_web::{HttpRequest, HttpResponse, web};
use chrono::Utc;
use dashmap::DashMap;
use rocksdb::DB;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::constants::CF_CONSUL_PEERING;

use crate::acl::{AclService, ResourceType};
use crate::consul_meta::{ConsulResponseMeta, consul_ok};
use crate::index_provider::{ConsulIndexProvider, ConsulTable};
use crate::model::ConsulError;
use crate::model::ConsulErrorBody;
use crate::raft::{ConsulRaftRequest, ConsulRaftWriter};

// ============================================================================
// Models
// ============================================================================

/// Peering state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[derive(Default)]
pub enum PeeringState {
    #[default]
/// The `Undefined` variant.
    Undefined,
/// The `Pending` variant.
    Pending,
/// The `Establishing` variant.
    Establishing,
/// The `Active` variant.
    Active,
/// The `Failing` variant.
    Failing,
/// The `Deleting` variant.
    Deleting,
/// The `Terminated` variant.
    Terminated,
}

impl std::fmt::Display for PeeringState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Undefined => write!(f, "UNDEFINED"),
            Self::Pending => write!(f, "PENDING"),
            Self::Establishing => write!(f, "ESTABLISHING"),
            Self::Active => write!(f, "ACTIVE"),
            Self::Failing => write!(f, "FAILING"),
            Self::Deleting => write!(f, "DELETING"),
            Self::Terminated => write!(f, "TERMINATED"),
        }
    }
}

/// Stream status for a peering
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PeeringStreamStatus {
/// The `imported_services` field.
    pub imported_services: Vec<String>,
/// The `exported_services` field.
    pub exported_services: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `last_heartbeat` field.
    pub last_heartbeat: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `last_receive` field.
    pub last_receive: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `last_send` field.
    pub last_send: Option<String>,
}

/// Remote peer info
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PeeringRemoteInfo {
    #[serde(default)]
/// The `partition` field.
    pub partition: String,
    #[serde(default)]
/// The `datacenter` field.
    pub datacenter: String,
}

/// A peering relationship
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Peering {
    #[serde(rename = "ID")]
/// The `id` field.
    pub id: String,
/// The `name` field.
    pub name: String,
    #[serde(default)]
/// The `partition` field.
    pub partition: String,
/// The `state` field.
    pub state: PeeringState,
    #[serde(rename = "PeerID")]
/// The `peer_id` field.
    pub peer_id: String,
    #[serde(default)]
/// The `peer_server_name` field.
    pub peer_server_name: String,
    #[serde(default)]
/// The `peer_server_addresses` field.
    pub peer_server_addresses: Vec<String>,
    #[serde(default, rename = "PeerCAPems")]
/// The `peer_ca_pems` field.
    pub peer_ca_pems: Vec<String>,
    #[serde(default)]
/// The `meta` field.
    pub meta: std::collections::HashMap<String, String>,
/// The `stream_status` field.
    pub stream_status: PeeringStreamStatus,
/// The `create_index` field.
    pub create_index: u64,
/// The `modify_index` field.
    pub modify_index: u64,
    #[serde(default)]
/// The `remote` field.
    pub remote: PeeringRemoteInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `deleted_at` field.
    pub deleted_at: Option<String>,
}

/// A service instance imported from a remote peer.
///
/// Stores a snapshot of a remote service's registration and health state so
/// that cross-peer health/catalog queries can return real data without a live
/// streaming connection. This is the Phase 1 data channel; Phase 2 will
/// populate it from an actual peering stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PeeringImportedService {
/// The `service_name` field.
    pub service_name: String,
/// The `service_id` field.
    #[serde(rename = "ServiceID")]
    pub service_id: String,
/// The `address` field.
    pub address: String,
/// The `port` field.
    pub port: u32,
    #[serde(default)]
/// The `tags` field.
    pub tags: Vec<String>,
    #[serde(default)]
/// The `meta` field.
    pub meta: std::collections::HashMap<String, String>,
    /// Remote datacenter this instance belongs to.
    #[serde(default)]
/// The `datacenter` field.
    pub datacenter: String,
    /// Remote node name hosting this instance.
    #[serde(default)]
/// The `node` field.
    pub node: String,
    /// Remote node address.
    #[serde(default)]
/// The `node_address` field.
    pub node_address: String,
    /// Health checks associated with this instance.
    #[serde(default)]
/// The `checks` field.
    pub checks: Vec<PeeringImportedCheck>,
    /// When this instance was imported (RFC3339).
    #[serde(default)]
/// The `imported_at` field.
    pub imported_at: String,
}

/// A health check snapshot imported alongside a service instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PeeringImportedCheck {
/// The `check_id` field.
    #[serde(rename = "CheckID")]
    pub check_id: String,
/// The `name` field.
    pub name: String,
/// The `status` field.
    pub status: String,
    #[serde(default)]
/// The `output` field.
    pub output: String,
}

/// Request to generate a peering token
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PeeringGenerateTokenRequest {
/// The `peer_name` field.
    pub peer_name: String,
    #[serde(default)]
/// The `partition` field.
    pub partition: String,
    #[serde(default)]
/// The `meta` field.
    pub meta: std::collections::HashMap<String, String>,
    #[serde(default)]
/// The `server_external_addresses` field.
    pub server_external_addresses: Vec<String>,
}

/// Response for generate token
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PeeringGenerateTokenResponse {
/// The `peering_token` field.
    pub peering_token: String,
}

/// Request to establish a peering
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PeeringEstablishRequest {
/// The `peer_name` field.
    pub peer_name: String,
/// The `peering_token` field.
    pub peering_token: String,
    #[serde(default)]
/// The `partition` field.
    pub partition: String,
    #[serde(default)]
/// The `meta` field.
    pub meta: std::collections::HashMap<String, String>,
}

/// Internal peering token structure
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PeeringToken {
    #[serde(rename = "CA")]
    ca: Vec<String>,
    server_addresses: Vec<String>,
    server_name: String,
    #[serde(rename = "PeerID")]
    peer_id: String,
    establishment_secret: String,
    remote: PeeringRemoteInfo,
}

/// Query parameters for peering endpoints
#[derive(Debug, Deserialize)]
pub struct PeeringQueryParams {
/// The `partition` field.
    pub partition: Option<String>,
}

// ============================================================================
// Service (In-Memory)
// ============================================================================

/// In-memory peering service
pub struct ConsulPeeringService {
    /// Peerings by name
    peerings: Arc<DashMap<String, Peering>>,
    /// Imported service instances keyed by peer name.
    ///
    /// Populated by the peering stream replication (Phase 1: via internal
    /// import API; Phase 2: via live stream). Read by cross-peer health and
    /// catalog queries.
    imported_instances: Arc<DashMap<String, Vec<PeeringImportedService>>>,
    /// Index counter
    index: std::sync::atomic::AtomicU64,
    /// Datacenter name
    datacenter: String,
    /// Consul compatibility HTTP port (default 8500)
    consul_port: u16,
    /// Optional RocksDB persistence
    rocks_db: Option<Arc<DB>>,
    /// Optional Raft writer for cluster-mode replication
    raft_node: Option<Arc<ConsulRaftWriter>>,
}

impl ConsulPeeringService {
/// The `new` associated function.
    pub fn new() -> Self {
        Self::with_datacenter("dc1".to_string())
    }

/// The `with_datacenter` associated function.
    pub fn with_datacenter(datacenter: String) -> Self {
        Self {
            peerings: Arc::new(DashMap::new()),
            imported_instances: Arc::new(DashMap::new()),
            index: std::sync::atomic::AtomicU64::new(1),
            datacenter,
            consul_port: 8500,
            rocks_db: None,
            raft_node: None,
        }
    }

/// The `with_consul_port` method.
    pub fn with_consul_port(mut self, port: u16) -> Self {
        self.consul_port = port;
        self
    }

/// The `with_rocks` associated function.
    pub fn with_rocks(db: Arc<DB>, datacenter: String, consul_port: u16) -> Self {
        let peerings = Arc::new(DashMap::new());
        let mut max_index = 1u64;

        // Load from RocksDB
        if let Some(cf) = db.cf_handle(CF_CONSUL_PEERING) {
            let iter = db.iterator_cf(cf, rocksdb::IteratorMode::Start);
            let mut count = 0u64;

            for item in iter.flatten() {
                let (key_bytes, value_bytes) = item;
                if let Ok(key) = String::from_utf8(key_bytes.to_vec()) {
                    if let Ok(peering) = serde_json::from_slice::<Peering>(&value_bytes) {
                        if peering.modify_index > max_index {
                            max_index = peering.modify_index;
                        }
                        peerings.insert(key, peering);
                        count += 1;
                    } else {
                        warn!(
                            "Failed to deserialize peering entry: {}",
                            String::from_utf8_lossy(&key_bytes)
                        );
                    }
                }
            }
            info!("Loaded {} peering entries from RocksDB", count);
        }

        Self {
            peerings,
            imported_instances: Arc::new(DashMap::new()),
            index: std::sync::atomic::AtomicU64::new(max_index + 1),
            datacenter,
            consul_port,
            rocks_db: Some(db),
            raft_node: None,
        }
    }

    /// Create a peering service with Raft-replicated storage (cluster mode).
    pub fn with_raft(
        db: Arc<DB>,
        raft_node: Arc<ConsulRaftWriter>,
        datacenter: String,
        consul_port: u16,
    ) -> Self {
        let mut svc = Self::with_rocks(db, datacenter, consul_port);
        svc.raft_node = Some(raft_node);
        svc
    }

/// The `generate_token` method.
    pub async fn generate_token(
        &self,
        req: PeeringGenerateTokenRequest,
    ) -> Result<PeeringGenerateTokenResponse, String> {
        if req.peer_name.is_empty() {
            return Err("PeerName is required".to_string());
        }

        let peer_id = uuid::Uuid::new_v4().to_string();
        let secret = uuid::Uuid::new_v4().to_string();
        let index = self.index.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        // Create the peering in PENDING state
        let peering = Peering {
            id: uuid::Uuid::new_v4().to_string(),
            name: req.peer_name.clone(),
            partition: req.partition,
            state: PeeringState::Pending,
            peer_id: peer_id.clone(),
            peer_server_name: String::new(),
            peer_server_addresses: Vec::new(),
            peer_ca_pems: Vec::new(),
            meta: req.meta,
            stream_status: PeeringStreamStatus::default(),
            create_index: index,
            modify_index: index,
            remote: PeeringRemoteInfo::default(),
            deleted_at: None,
        };
        self.peerings.insert(req.peer_name.clone(), peering.clone());
        if let Some(ref raft) = self.raft_node {
            let peering_json = serde_json::to_string(&peering).unwrap_or_default();
            match raft
                .write(ConsulRaftRequest::PeeringWrite {
                    name: req.peer_name.clone(),
                    peering_json,
                })
                .await
            {
                Ok(r) if !r.success => {
                    error!("Raft PeeringWrite rejected: {:?}", r.message);
                }
                Err(e) => {
                    error!("Raft PeeringWrite failed: {}", e);
                }
                _ => {}
            }
        } else {
            self.persist_to_rocks(&req.peer_name, &peering);
        }

        // Generate the token
        let token = PeeringToken {
            ca: Vec::new(),
            server_addresses: if req.server_external_addresses.is_empty() {
                let hostname = hostname::get()
                    .map(|h| h.to_string_lossy().to_string())
                    .unwrap_or_else(|_| "127.0.0.1".to_string());
                vec![format!("{}:{}", hostname, self.consul_port)]
            } else {
                req.server_external_addresses
            },
            server_name: format!("server.{}.consul", self.datacenter),
            peer_id,
            establishment_secret: secret,
            remote: PeeringRemoteInfo {
                partition: "default".to_string(),
                datacenter: self.datacenter.clone(),
            },
        };

        let token_json = serde_json::to_vec(&token).map_err(|e| e.to_string())?;
        let token_b64 =
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &token_json);

        Ok(PeeringGenerateTokenResponse {
            peering_token: token_b64,
        })
    }

/// The `establish` method.
    pub async fn establish(&self, req: PeeringEstablishRequest) -> Result<(), String> {
        if req.peer_name.is_empty() {
            return Err("PeerName is required".to_string());
        }
        if req.peering_token.is_empty() {
            return Err("PeeringToken is required".to_string());
        }

        // Decode the token
        let token_bytes = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            &req.peering_token,
        )
        .map_err(|e| format!("Invalid peering token: {}", e))?;

        let token: PeeringToken = serde_json::from_slice(&token_bytes)
            .map_err(|e| format!("Invalid token data: {}", e))?;

        let index = self.index.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let now = Utc::now().to_rfc3339();

        let peering = Peering {
            id: uuid::Uuid::new_v4().to_string(),
            name: req.peer_name.clone(),
            partition: req.partition,
            state: PeeringState::Active,
            peer_id: token.peer_id,
            peer_server_name: token.server_name,
            peer_server_addresses: token.server_addresses,
            peer_ca_pems: token.ca,
            meta: req.meta,
            stream_status: PeeringStreamStatus {
                imported_services: Vec::new(),
                exported_services: Vec::new(),
                last_heartbeat: Some(now.clone()),
                last_receive: Some(now.clone()),
                last_send: Some(now),
            },
            create_index: index,
            modify_index: index,
            remote: token.remote,
            deleted_at: None,
        };

        self.peerings.insert(req.peer_name.clone(), peering.clone());
        if let Some(ref raft) = self.raft_node {
            let peering_json = serde_json::to_string(&peering).unwrap_or_default();
            match raft
                .write(ConsulRaftRequest::PeeringWrite {
                    name: req.peer_name.clone(),
                    peering_json,
                })
                .await
            {
                Ok(r) if !r.success => {
                    error!("Raft PeeringWrite rejected: {:?}", r.message);
                }
                Err(e) => {
                    error!("Raft PeeringWrite failed: {}", e);
                }
                _ => {}
            }
        } else {
            self.persist_to_rocks(&req.peer_name, &peering);
        }
        Ok(())
    }

/// The `get_peering` method.
    pub fn get_peering(&self, name: &str) -> Option<Peering> {
        self.peerings
            .get(name)
            .filter(|p| p.deleted_at.is_none())
            .map(|p| p.value().clone())
    }

/// The `list_peerings` method.
    pub fn list_peerings(&self) -> Vec<Peering> {
        let mut peerings: Vec<Peering> = self
            .peerings
            .iter()
            .filter(|r| r.value().deleted_at.is_none())
            .map(|r| r.value().clone())
            .collect();
        peerings.sort_by(|a, b| a.name.cmp(&b.name));
        peerings
    }

/// The `delete_peering` method.
    pub async fn delete_peering(&self, name: &str) -> bool {
        if let Some(mut peering) = self.peerings.get_mut(name) {
            peering.state = PeeringState::Deleting;
            peering.deleted_at = Some(Utc::now().to_rfc3339());
            peering.modify_index = self.index.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let updated = peering.clone();
            drop(peering);
            if let Some(ref raft) = self.raft_node {
                let peering_json = serde_json::to_string(&updated).unwrap_or_default();
                match raft
                    .write(ConsulRaftRequest::PeeringWrite {
                        name: name.to_string(),
                        peering_json,
                    })
                    .await
                {
                    Ok(r) if !r.success => {
                        error!("Raft PeeringWrite rejected: {:?}", r.message);
                    }
                    Err(e) => {
                        error!("Raft PeeringWrite failed: {}", e);
                    }
                    _ => {}
                }
            } else {
                self.persist_to_rocks(name, &updated);
            }
            true
        } else {
            false
        }
    }

    // ========================================================================
    // Imported service replication (Phase 1 data channel)
    // ========================================================================

    /// Import a batch of service instances from a remote peer.
    ///
    /// Replaces any previously imported instances for this peer. The peering's
    /// `stream_status.imported_services` list is updated to reflect the
    /// distinct service names now imported, and `last_receive` is refreshed.
    ///
    /// Returns `Ok(count)` on success, or an error if the peering does not
    /// exist or is not active.
    pub async fn import_services(
        &self,
        peer_name: &str,
        services: Vec<PeeringImportedService>,
    ) -> Result<usize, String> {
        let now = Utc::now().to_rfc3339();
        let count = services.len();

        // Update the imported instances store.
        self.imported_instances
            .insert(peer_name.to_string(), services.clone());

        // Update the peering's stream_status: imported_services list + last_receive.
        if let Some(mut peering) = self.peerings.get_mut(peer_name) {
            let mut service_names: Vec<String> = services
                .iter()
                .map(|s| s.service_name.clone())
                .collect();
            service_names.sort();
            service_names.dedup();

            peering.stream_status.imported_services = service_names;
            peering.stream_status.last_receive = Some(now.clone());
            peering.modify_index = self.index.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let updated = peering.clone();
            drop(peering);

            if let Some(ref raft) = self.raft_node {
                let peering_json = serde_json::to_string(&updated).unwrap_or_default();
                match raft
                    .write(ConsulRaftRequest::PeeringWrite {
                        name: peer_name.to_string(),
                        peering_json,
                    })
                    .await
                {
                    Ok(r) if !r.success => {
                        error!("Raft PeeringWrite (import) rejected: {:?}", r.message);
                    }
                    Err(e) => {
                        error!("Raft PeeringWrite (import) failed: {}", e);
                    }
                    _ => {}
                }
            } else {
                self.persist_to_rocks(peer_name, &updated);
            }
        } else {
            // Peering not found — still store the instances but warn.
            warn!(
                "import_services: peering '{}' not found; instances stored but stream_status not updated",
                peer_name
            );
        }

        info!(
            "Imported {} service instances from peer '{}'",
            count, peer_name
        );
        Ok(count)
    }

    /// Get all imported service instances for a peer.
    pub fn get_imported_services(&self, peer_name: &str) -> Vec<PeeringImportedService> {
        self.imported_instances
            .get(peer_name)
            .map(|r| r.value().clone())
            .unwrap_or_default()
    }

    /// Get imported service instances for a specific service from a peer.
    pub fn get_imported_service_instances(
        &self,
        peer_name: &str,
        service_name: &str,
    ) -> Vec<PeeringImportedService> {
        self.imported_instances
            .get(peer_name)
            .map(|r| {
                r.value()
                    .iter()
                    .filter(|s| s.service_name == service_name)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Collect all imported service names across all active peerings.
    ///
    /// Returns a list of `(service_name, peer_name)` pairs derived from each
    /// active peering's `stream_status.imported_services`. Used by the connect
    /// module to populate `/v1/imported-services` from the single source of
    /// truth maintained by the peering service.
    pub fn list_all_imported_service_names(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for entry in self.peerings.iter() {
            let peering = entry.value();
            if peering.state != PeeringState::Active || peering.deleted_at.is_some() {
                continue;
            }
            for svc in &peering.stream_status.imported_services {
                out.push((svc.clone(), peering.name.clone()));
            }
        }
        out.sort();
        out
    }

    /// Clear all imported service instances for a peer.
    pub async fn clear_imported_services(&self, peer_name: &str) {
        self.imported_instances.remove(peer_name);
        if let Some(mut peering) = self.peerings.get_mut(peer_name) {
            peering.stream_status.imported_services.clear();
            peering.modify_index = self.index.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let updated = peering.clone();
            drop(peering);
            if let Some(ref raft) = self.raft_node {
                let peering_json = serde_json::to_string(&updated).unwrap_or_default();
                match raft
                    .write(ConsulRaftRequest::PeeringWrite {
                        name: peer_name.to_string(),
                        peering_json,
                    })
                    .await
                {
                    Ok(r) if !r.success => {
                        error!("Raft PeeringWrite (clear import) rejected: {:?}", r.message);
                    }
                    Err(e) => {
                        error!("Raft PeeringWrite (clear import) failed: {}", e);
                    }
                    _ => {}
                }
            } else {
                self.persist_to_rocks(peer_name, &updated);
            }
        }
        info!("Cleared imported services for peer '{}'", peer_name);
    }

    /// Update the heartbeat timestamp for a peering's stream.
    pub fn update_stream_heartbeat(&self, peer_name: &str) {
        if let Some(mut peering) = self.peerings.get_mut(peer_name) {
            let now = Utc::now().to_rfc3339();
            peering.stream_status.last_heartbeat = Some(now);
            peering.modify_index = self.index.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }

    // ========================================================================
    // Phase 2: network replication (pull mode)
    // ========================================================================

    /// Replicate imported service instances from a remote peer by polling its
    /// Consul-compatible HTTP API.
    ///
    /// Phase 2 uses a pull model: for each active peering with known server
    /// addresses, periodically fetch the remote service catalog and health
    /// data, then store it locally so cross-peer queries return real instances.
    ///
    /// Returns the number of imported instances on success.
    pub async fn replicate_from_peer(&self, peer_name: &str) -> Result<usize, String> {
        let peering = self
            .get_peering(peer_name)
            .ok_or_else(|| format!("peering '{}' not found", peer_name))?;

        if peering.state != PeeringState::Active {
            return Err(format!("peering '{}' is not active", peer_name));
        }
        if peering.peer_server_addresses.is_empty() {
            return Err(format!(
                "peering '{}' has no server addresses to replicate from",
                peer_name
            ));
        }

        let client = PeeringReplicationClient::new();
        let addr = &peering.peer_server_addresses[0];
        let base_url = format!("http://{}", addr);

        // Step 1: discover which services to replicate.
        let service_names = client.fetch_service_names(&base_url).await.map_err(|e| {
            format!(
                "failed to fetch service names from peer '{}' ({}): {}",
                peer_name, base_url, e
            )
        })?;

        if service_names.is_empty() {
            tracing::debug!(
                peer = peer_name,
                "no services to replicate from remote peer"
            );
            // Still import an empty set to clear stale instances.
            return self.import_services(peer_name, Vec::new()).await;
        }

        // Step 2: fetch health instances for each service and convert.
        let mut imported: Vec<PeeringImportedService> = Vec::new();
        for svc in &service_names {
            match client.fetch_service_health(&base_url, svc).await {
                Ok(instances) => {
                    for sh in &instances {
                        imported.push(convert_service_health(sh, peer_name));
                    }
                }
                Err(e) => {
                    warn!(
                        peer = peer_name,
                        service = svc,
                        "failed to fetch health for service: {}",
                        e
                    );
                }
            }
        }

        let count = self.import_services(peer_name, imported).await?;
        info!(
            "Replicated {} service instances from peer '{}' ({} services)",
            count,
            peer_name,
            service_names.len()
        );
        Ok(count)
    }

    /// Persist a peering entry to RocksDB
    fn persist_to_rocks(&self, name: &str, peering: &Peering) {
        if let Some(ref db) = self.rocks_db
            && let Some(cf) = db.cf_handle(CF_CONSUL_PEERING)
        {
            match serde_json::to_vec(peering) {
                Ok(bytes) => {
                    if let Err(e) = db.put_cf(cf, name.as_bytes(), &bytes) {
                        error!("Failed to persist peering '{}': {}", name, e);
                    }
                }
                Err(e) => {
                    error!("Failed to serialize peering '{}': {}", name, e);
                }
            }
        }
    }

    /// Delete a peering entry from RocksDB
    #[allow(dead_code)]
    fn delete_from_rocks(&self, name: &str) {
        if let Some(ref db) = self.rocks_db
            && let Some(cf) = db.cf_handle(CF_CONSUL_PEERING)
            && let Err(e) = db.delete_cf(cf, name.as_bytes())
        {
            error!("Failed to delete peering '{}': {}", name, e);
        }
    }
}

impl Default for ConsulPeeringService {
    fn default() -> Self {
        Self::new()
    }
}

impl ConsulPeeringService {
    /// Test-only accessor for the underlying peerings map.
    ///
    /// Allows unit tests to seed peering entries directly without going through
    /// the token/establish flow.
    #[cfg(test)]
    pub fn peerings_for_test(&self) -> &Arc<DashMap<String, Peering>> {
        &self.peerings
    }

    /// Test-only accessor for the imported instances map.
    #[cfg(test)]
    pub fn imported_instances_for_test(
        &self,
    ) -> &Arc<DashMap<String, Vec<PeeringImportedService>>> {
        &self.imported_instances
    }
}

// ============================================================================
// Phase 2: replication client & conversion
// ============================================================================

/// HTTP client for polling a remote peer's Consul-compatible API during
/// Phase 2 pull-mode replication.
///
/// Wraps `reqwest::Client` with the specific endpoints used for service
/// discovery and health fetching.
#[derive(Clone)]
pub struct PeeringReplicationClient {
    http: reqwest::Client,
}

impl PeeringReplicationClient {
/// The `new` associated function.
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("failed to build peering replication http client");
        Self { http }
    }

    /// Fetch the list of service names available from a remote peer.
    ///
    /// Strategy:
    /// 1. Try `GET /v1/exported-services` (Consul semantics — only services
    ///    explicitly exported by the remote peer are replicated).
    /// 2. If that returns an empty list, fall back to `GET /v1/catalog/services`
    ///    and replicate every service registered on the remote.
    pub async fn fetch_service_names(&self, base_url: &str) -> Result<Vec<String>, String> {
        // 1. Exported services (preferred — correct Consul semantics).
        let exported_url = format!("{}/v1/exported-services", base_url);
        if let Ok(resp) = self.http.get(&exported_url).send().await {
            if resp.status().is_success() {
                if let Ok(services) = resp
                    .json::<Vec<crate::connect::ResolvedExportedService>>()
                    .await
                {
                    if !services.is_empty() {
                        let names: Vec<String> =
                            services.into_iter().map(|s| s.service).collect();
                        return Ok(names);
                    }
                }
            }
        }

        // 2. Fallback: all registered services.
        let catalog_url = format!("{}/v1/catalog/services", base_url);
        let resp = self
            .http
            .get(&catalog_url)
            .send()
            .await
            .map_err(|e| format!("catalog/services request failed: {}", e))?;
        if !resp.status().is_success() {
            return Err(format!(
                "catalog/services returned status {}",
                resp.status()
            ));
        }
        let map: std::collections::BTreeMap<String, Vec<String>> = resp
            .json()
            .await
            .map_err(|e| format!("failed to parse catalog/services response: {}", e))?;
        Ok(map.into_keys().collect())
    }

    /// Fetch all health instances for a given service from the remote peer.
    pub async fn fetch_service_health(
        &self,
        base_url: &str,
        service: &str,
    ) -> Result<Vec<crate::model::ServiceHealth>, String> {
        let url = format!("{}/v1/health/service/{}", base_url, service);
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("health/service request failed: {}", e))?;
        if !resp.status().is_success() {
            return Err(format!(
                "health/service/{} returned status {}",
                service,
                resp.status()
            ));
        }
        resp.json::<Vec<crate::model::ServiceHealth>>()
            .await
            .map_err(|e| format!("failed to parse health/service response: {}", e))
    }
}

impl Default for PeeringReplicationClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert a remote `ServiceHealth` entry into a `PeeringImportedService` for
/// local storage.
pub fn convert_service_health(
    sh: &crate::model::ServiceHealth,
    _peer_name: &str,
) -> PeeringImportedService {
    let checks: Vec<PeeringImportedCheck> = sh
        .checks
        .iter()
        .map(|c| PeeringImportedCheck {
            check_id: c.check_id.clone(),
            name: c.name.clone(),
            status: c.status.clone(),
            output: c.output.clone(),
        })
        .collect();

    PeeringImportedService {
        service_name: sh.service.service.clone(),
        service_id: sh.service.id.clone(),
        address: sh.service.address.clone(),
        port: sh.service.port as u32,
        tags: sh.service.tags.clone().unwrap_or_default(),
        meta: sh.service.meta.clone().unwrap_or_default(),
        datacenter: sh.node.datacenter.clone(),
        node: sh.node.node.clone(),
        node_address: sh.node.address.clone(),
        checks,
        imported_at: Utc::now().to_rfc3339(),
    }
}

// ============================================================================
// HTTP Handlers (In-Memory)
// ============================================================================

/// POST /v1/peering/token - Generate a peering token
pub async fn generate_peering_token(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    peering_service: web::Data<ConsulPeeringService>,
    body: web::Json<PeeringGenerateTokenRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    match peering_service.generate_token(body.into_inner()).await {
        Ok(resp) => {
            let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Peering));
            consul_ok(&meta).json(resp)
        }
        Err(e) => HttpResponse::BadRequest().consul_error(ConsulError::new(e)),
    }
}

/// POST /v1/peering/establish - Establish a peering
pub async fn establish_peering(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    peering_service: web::Data<ConsulPeeringService>,
    body: web::Json<PeeringEstablishRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    match peering_service.establish(body.into_inner()).await {
        Ok(()) => {
            let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Peering));
            consul_ok(&meta).json(serde_json::json!({}))
        }
        Err(e) => HttpResponse::BadRequest().consul_error(ConsulError::new(e)),
    }
}

/// GET /v1/peering/{name} - Read a peering
pub async fn get_peering(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    peering_service: web::Data<ConsulPeeringService>,
    path: web::Path<String>,
    _query: web::Query<PeeringQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let name = path.into_inner();
    if name.is_empty() {
        return HttpResponse::BadRequest()
            .consul_error(ConsulError::new("Peering name is required"));
    }

    match peering_service.get_peering(&name) {
        Some(peering) => {
            let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Peering));
            consul_ok(&meta).json(peering)
        }
        None => HttpResponse::NotFound()
            .consul_error(ConsulError::new(format!("Peering '{}' not found", name))),
    }
}

/// DELETE /v1/peering/{name} - Delete a peering
pub async fn delete_peering(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    peering_service: web::Data<ConsulPeeringService>,
    path: web::Path<String>,
    _query: web::Query<PeeringQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let name = path.into_inner();
    if name.is_empty() {
        return HttpResponse::BadRequest()
            .consul_error(ConsulError::new("Peering name is required"));
    }

    if peering_service.delete_peering(&name).await {
        let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Peering));
        consul_ok(&meta).finish()
    } else {
        HttpResponse::NotFound()
            .consul_error(ConsulError::new(format!("Peering '{}' not found", name)))
    }
}

/// GET /v1/peerings - List all peerings
pub async fn list_peerings(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    peering_service: web::Data<ConsulPeeringService>,
    _query: web::Query<PeeringQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Peering));
    consul_ok(&meta).json(peering_service.list_peerings())
}

/// Response for the peering import endpoint.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PeeringImportResponse {
/// The `imported` field.
    pub imported: usize,
}

/// POST /v1/internal/peering/{name}/import - Import remote service instances
///
/// Phase 1 data channel: accepts a batch of `PeeringImportedService` instances
/// from a remote peer and stores them so cross-peer health/catalog queries can
/// return real data. The peering must exist and be `Active`.
pub async fn import_peering_services(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    peering_service: web::Data<ConsulPeeringService>,
    path: web::Path<String>,
    body: web::Json<Vec<PeeringImportedService>>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let peer_name = path.into_inner();
    if peer_name.is_empty() {
        return HttpResponse::BadRequest()
            .consul_error(ConsulError::new("Peering name is required"));
    }

    // Validate the peering exists and is active.
    match peering_service.get_peering(&peer_name) {
        Some(p) if p.state == PeeringState::Active => {}
        Some(_) => {
            return HttpResponse::BadRequest().consul_error(ConsulError::new(format!(
                "Peering '{}' is not active",
                peer_name
            )));
        }
        None => {
            return HttpResponse::NotFound().consul_error(ConsulError::new(format!(
                "Peering '{}' not found",
                peer_name
            )));
        }
    }

    let services = body.into_inner();
    match peering_service.import_services(&peer_name, services).await {
        Ok(count) => {
            let meta =
                ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Peering));
            consul_ok(&meta).json(PeeringImportResponse { imported: count })
        }
        Err(e) => HttpResponse::BadRequest().consul_error(ConsulError::new(e)),
    }
}

// ============================================================================
// HTTP Handlers (Persistent)
// ============================================================================

/// POST /v1/peering/token (persistent)
pub async fn generate_peering_token_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    peering_service: web::Data<ConsulPeeringService>,
    body: web::Json<PeeringGenerateTokenRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    match peering_service.generate_token(body.into_inner()).await {
        Ok(resp) => {
            let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Peering));
            consul_ok(&meta).json(resp)
        }
        Err(e) => HttpResponse::BadRequest().consul_error(ConsulError::new(e)),
    }
}

/// POST /v1/peering/establish (persistent)
pub async fn establish_peering_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    peering_service: web::Data<ConsulPeeringService>,
    body: web::Json<PeeringEstablishRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    match peering_service.establish(body.into_inner()).await {
        Ok(()) => {
            let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Peering));
            consul_ok(&meta).json(serde_json::json!({}))
        }
        Err(e) => HttpResponse::BadRequest().consul_error(ConsulError::new(e)),
    }
}

/// GET /v1/peering/{name} (persistent)
pub async fn get_peering_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    peering_service: web::Data<ConsulPeeringService>,
    path: web::Path<String>,
    _query: web::Query<PeeringQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let name = path.into_inner();
    if name.is_empty() {
        return HttpResponse::BadRequest()
            .consul_error(ConsulError::new("Peering name is required"));
    }

    match peering_service.get_peering(&name) {
        Some(peering) => {
            let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Peering));
            consul_ok(&meta).json(peering)
        }
        None => HttpResponse::NotFound()
            .consul_error(ConsulError::new(format!("Peering '{}' not found", name))),
    }
}

/// DELETE /v1/peering/{name} (persistent)
pub async fn delete_peering_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    peering_service: web::Data<ConsulPeeringService>,
    path: web::Path<String>,
    _query: web::Query<PeeringQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let name = path.into_inner();
    if name.is_empty() {
        return HttpResponse::BadRequest()
            .consul_error(ConsulError::new("Peering name is required"));
    }

    if peering_service.delete_peering(&name).await {
        let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Peering));
        consul_ok(&meta).finish()
    } else {
        HttpResponse::NotFound()
            .consul_error(ConsulError::new(format!("Peering '{}' not found", name)))
    }
}

/// GET /v1/peerings (persistent)
pub async fn list_peerings_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    peering_service: web::Data<ConsulPeeringService>,
    _query: web::Query<PeeringQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Peering));
    consul_ok(&meta).json(peering_service.list_peerings())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_generate_token() {
        let service = ConsulPeeringService::new();
        let result = service
            .generate_token(PeeringGenerateTokenRequest {
                peer_name: "cluster-02".to_string(),
                partition: String::new(),
                meta: Default::default(),
                server_external_addresses: vec![],
            })
            .await;
        assert!(result.is_ok());
        let resp = result.unwrap();
        assert!(!resp.peering_token.is_empty());

        // Peering should be in PENDING state
        let peering = service.get_peering("cluster-02").unwrap();
        assert_eq!(peering.state, PeeringState::Pending);
    }

    #[tokio::test]
    async fn test_generate_token_empty_name() {
        let service = ConsulPeeringService::new();
        let result = service
            .generate_token(PeeringGenerateTokenRequest {
                peer_name: String::new(),
                partition: String::new(),
                meta: Default::default(),
                server_external_addresses: vec![],
            })
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_establish_peering() {
        let service = ConsulPeeringService::new();

        // First generate a token
        let token_resp = service
            .generate_token(PeeringGenerateTokenRequest {
                peer_name: "cluster-02".to_string(),
                partition: String::new(),
                meta: Default::default(),
                server_external_addresses: vec![],
            })
            .await
            .unwrap();

        // Create another service and establish
        let service2 = ConsulPeeringService::new();
        let result = service2
            .establish(PeeringEstablishRequest {
                peer_name: "cluster-01".to_string(),
                peering_token: token_resp.peering_token,
                partition: String::new(),
                meta: Default::default(),
            })
            .await;
        assert!(result.is_ok());

        let peering = service2.get_peering("cluster-01").unwrap();
        assert_eq!(peering.state, PeeringState::Active);
    }

    #[tokio::test]
    async fn test_list_peerings() {
        let service = ConsulPeeringService::new();
        service
            .generate_token(PeeringGenerateTokenRequest {
                peer_name: "peer-b".to_string(),
                partition: String::new(),
                meta: Default::default(),
                server_external_addresses: vec![],
            })
            .await
            .unwrap();
        service
            .generate_token(PeeringGenerateTokenRequest {
                peer_name: "peer-a".to_string(),
                partition: String::new(),
                meta: Default::default(),
                server_external_addresses: vec![],
            })
            .await
            .unwrap();

        let peerings = service.list_peerings();
        assert_eq!(peerings.len(), 2);
        // Should be sorted by name
        assert_eq!(peerings[0].name, "peer-a");
        assert_eq!(peerings[1].name, "peer-b");
    }

    #[tokio::test]
    async fn test_delete_peering() {
        let service = ConsulPeeringService::new();
        service
            .generate_token(PeeringGenerateTokenRequest {
                peer_name: "to-delete".to_string(),
                partition: String::new(),
                meta: Default::default(),
                server_external_addresses: vec![],
            })
            .await
            .unwrap();

        assert!(service.delete_peering("to-delete").await);
        // Should not appear in list after deletion
        assert!(service.get_peering("to-delete").is_none());
        assert!(service.list_peerings().is_empty());
    }

    #[tokio::test]
    async fn test_delete_nonexistent() {
        let service = ConsulPeeringService::new();
        assert!(!service.delete_peering("nonexistent").await);
    }

    #[test]
    fn test_get_nonexistent_peering() {
        let service = ConsulPeeringService::new();
        assert!(service.get_peering("nonexistent").is_none());
    }

    #[tokio::test]
    async fn test_generate_token_creates_pending_peering() {
        let service = ConsulPeeringService::new();
        service
            .generate_token(PeeringGenerateTokenRequest {
                peer_name: "pending-peer".to_string(),
                partition: String::new(),
                meta: Default::default(),
                server_external_addresses: vec![],
            })
            .await
            .unwrap();

        let peering = service.get_peering("pending-peer").unwrap();
        assert_eq!(peering.state, PeeringState::Pending);
        assert_eq!(peering.name, "pending-peer");
        assert!(!peering.id.is_empty());
    }

    #[tokio::test]
    async fn test_generate_token_duplicate_name_overwrites() {
        let service = ConsulPeeringService::new();

        service
            .generate_token(PeeringGenerateTokenRequest {
                peer_name: "dup-peer".to_string(),
                partition: String::new(),
                meta: Default::default(),
                server_external_addresses: vec![],
            })
            .await
            .unwrap();

        // Second token for same name overwrites (insert into DashMap)
        let result = service
            .generate_token(PeeringGenerateTokenRequest {
                peer_name: "dup-peer".to_string(),
                partition: String::new(),
                meta: Default::default(),
                server_external_addresses: vec![],
            })
            .await;
        assert!(result.is_ok());

        // Should still only be one peering
        assert_eq!(
            service
                .list_peerings()
                .iter()
                .filter(|p| p.name == "dup-peer")
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn test_establish_with_invalid_token() {
        let service = ConsulPeeringService::new();

        let result = service
            .establish(PeeringEstablishRequest {
                peer_name: "bad-peer".to_string(),
                peering_token: "not-valid-base64!@#$".to_string(),
                partition: String::new(),
                meta: Default::default(),
            })
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_peering_with_meta() {
        let service = ConsulPeeringService::new();
        let mut meta = std::collections::HashMap::new();
        meta.insert("env".to_string(), "production".to_string());

        service
            .generate_token(PeeringGenerateTokenRequest {
                peer_name: "meta-peer".to_string(),
                partition: String::new(),
                meta,
                server_external_addresses: vec![],
            })
            .await
            .unwrap();

        let peering = service.get_peering("meta-peer").unwrap();
        assert_eq!(peering.meta.get("env").unwrap(), "production");
    }

    #[tokio::test]
    async fn test_generate_token_with_external_addresses() {
        let service = ConsulPeeringService::new();

        let resp = service
            .generate_token(PeeringGenerateTokenRequest {
                peer_name: "addr-peer".to_string(),
                partition: String::new(),
                meta: Default::default(),
                server_external_addresses: vec![
                    "10.0.1.1:8502".to_string(),
                    "10.0.1.2:8502".to_string(),
                ],
            })
            .await
            .unwrap();

        // Token should contain the addresses (base64 encoded)
        assert!(!resp.peering_token.is_empty());

        // The peering itself stores addresses in the token, not in peer_server_addresses
        // peer_server_addresses is populated when establishing from the remote side
        let peering = service.get_peering("addr-peer").unwrap();
        assert_eq!(peering.state, PeeringState::Pending);
    }

    // ========================================================================
    // Imported service replication tests
    // ========================================================================

    fn make_active_peering(service: &ConsulPeeringService, name: &str) {
        let peering = Peering {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.to_string(),
            partition: String::new(),
            state: PeeringState::Active,
            peer_id: uuid::Uuid::new_v4().to_string(),
            peer_server_name: String::new(),
            peer_server_addresses: Vec::new(),
            peer_ca_pems: Vec::new(),
            meta: Default::default(),
            stream_status: PeeringStreamStatus::default(),
            create_index: 1,
            modify_index: 1,
            remote: PeeringRemoteInfo {
                partition: "default".to_string(),
                datacenter: "dc-remote".to_string(),
            },
            deleted_at: None,
        };
        service.peerings_for_test().insert(name.to_string(), peering);
    }

    fn sample_imported_service(name: &str, id: &str) -> PeeringImportedService {
        PeeringImportedService {
            service_name: name.to_string(),
            service_id: id.to_string(),
            address: "10.0.0.5".to_string(),
            port: 8080,
            tags: vec!["v1".to_string()],
            meta: Default::default(),
            datacenter: "dc-remote".to_string(),
            node: "remote-node".to_string(),
            node_address: "10.0.0.5".to_string(),
            checks: vec![PeeringImportedCheck {
                check_id: format!("check-{}", id),
                name: format!("check-{}", name),
                status: "passing".to_string(),
                output: String::new(),
            }],
            imported_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[tokio::test]
    async fn test_import_services_stores_instances_and_updates_stream_status() {
        let service = ConsulPeeringService::new();
        make_active_peering(&service, "peer-b");

        let instances = vec![
            sample_imported_service("web", "web-1"),
            sample_imported_service("web", "web-2"),
            sample_imported_service("api", "api-1"),
        ];
        let count = service.import_services("peer-b", instances).await.unwrap();
        assert_eq!(count, 3);

        // stream_status.imported_services should list distinct service names.
        let peering = service.get_peering("peer-b").unwrap();
        let mut imported = peering.stream_status.imported_services.clone();
        imported.sort();
        assert_eq!(imported, vec!["api".to_string(), "web".to_string()]);
        assert!(peering.stream_status.last_receive.is_some());

        // get_imported_services returns all instances for the peer.
        let all = service.get_imported_services("peer-b");
        assert_eq!(all.len(), 3);

        // get_imported_service_instances filters by service name.
        let web_instances = service.get_imported_service_instances("peer-b", "web");
        assert_eq!(web_instances.len(), 2);
        let api_instances = service.get_imported_service_instances("peer-b", "api");
        assert_eq!(api_instances.len(), 1);
    }

    #[tokio::test]
    async fn test_import_services_replaces_previous_batch() {
        let service = ConsulPeeringService::new();
        make_active_peering(&service, "peer-b");

        service
            .import_services("peer-b", vec![sample_imported_service("web", "web-1")])
            .await
            .unwrap();
        assert_eq!(service.get_imported_services("peer-b").len(), 1);

        // Second import replaces the first batch entirely.
        service
            .import_services("peer-b", vec![sample_imported_service("db", "db-1")])
            .await
            .unwrap();
        let all = service.get_imported_services("peer-b");
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].service_name, "db");

        let peering = service.get_peering("peer-b").unwrap();
        assert_eq!(peering.stream_status.imported_services, vec!["db"]);
    }

    #[tokio::test]
    async fn test_clear_imported_services() {
        let service = ConsulPeeringService::new();
        make_active_peering(&service, "peer-b");

        service
            .import_services("peer-b", vec![sample_imported_service("web", "web-1")])
            .await
            .unwrap();
        assert!(!service.get_imported_services("peer-b").is_empty());

        service.clear_imported_services("peer-b").await;
        assert!(service.get_imported_services("peer-b").is_empty());

        let peering = service.get_peering("peer-b").unwrap();
        assert!(peering.stream_status.imported_services.is_empty());
    }

    #[test]
    fn test_update_stream_heartbeat() {
        let service = ConsulPeeringService::new();
        make_active_peering(&service, "peer-b");

        let before = service
            .get_peering("peer-b")
            .unwrap()
            .stream_status
            .last_heartbeat
            .clone();
        assert!(before.is_none());

        service.update_stream_heartbeat("peer-b");

        let after = service
            .get_peering("peer-b")
            .unwrap()
            .stream_status
            .last_heartbeat
            .clone();
        assert!(after.is_some());
    }

    #[test]
    fn test_list_all_imported_service_names() {
        let service = ConsulPeeringService::new();
        make_active_peering(&service, "peer-b");

        // Manually set imported_services on the peering.
        {
            let mut peering = service.peerings_for_test().get_mut("peer-b").unwrap();
            peering.stream_status.imported_services =
                vec!["web".to_string(), "api".to_string()];
        }

        let names = service.list_all_imported_service_names();
        // Sorted by (service, peer).
        assert_eq!(
            names,
            vec![
                ("api".to_string(), "peer-b".to_string()),
                ("web".to_string(), "peer-b".to_string()),
            ]
        );
    }

    #[test]
    fn test_list_all_imported_service_names_skips_inactive() {
        let service = ConsulPeeringService::new();
        let peering = Peering {
            id: uuid::Uuid::new_v4().to_string(),
            name: "pending-peer".to_string(),
            partition: String::new(),
            state: PeeringState::Pending,
            peer_id: uuid::Uuid::new_v4().to_string(),
            peer_server_name: String::new(),
            peer_server_addresses: Vec::new(),
            peer_ca_pems: Vec::new(),
            meta: Default::default(),
            stream_status: PeeringStreamStatus {
                imported_services: vec!["web".to_string()],
                ..Default::default()
            },
            create_index: 1,
            modify_index: 1,
            remote: PeeringRemoteInfo::default(),
            deleted_at: None,
        };
        service
            .peerings_for_test()
            .insert("pending-peer".to_string(), peering);

        assert!(service.list_all_imported_service_names().is_empty());
    }

    // ========================================================================
    // Phase 2 replication tests
    // ========================================================================

    #[test]
    fn test_convert_service_health_maps_all_fields() {
        use crate::model::{AgentService, HealthCheck, Node, ServiceHealth};
        use std::collections::HashMap;

        let sh = ServiceHealth {
            node: Node {
                id: "node-1".to_string(),
                node: "remote-node".to_string(),
                address: "10.0.0.5".to_string(),
                datacenter: "dc-remote".to_string(),
                tagged_addresses: None,
                meta: None,
                create_index: 1,
                modify_index: 1,
            },
            service: AgentService {
                id: "web-1".to_string(),
                service: "web".to_string(),
                tags: Some(vec!["v1".to_string(), "prod".to_string()]),
                port: 8080,
                address: "10.0.0.5".to_string(),
                meta: Some(HashMap::from([("env".to_string(), "prod".to_string())])),
                enable_tag_override: false,
                weights: Default::default(),
                datacenter: Some("dc-remote".to_string()),
                kind: None,
                proxy: None,
                connect: None,
                tagged_addresses: None,
                namespace: None,
                peer_name: None,
                create_index: None,
                modify_index: None,
                socket_path: None,
            },
            checks: vec![HealthCheck {
                node: "remote-node".to_string(),
                check_id: "check-web-1".to_string(),
                name: "web health".to_string(),
                status: "passing".to_string(),
                notes: String::new(),
                output: "ok".to_string(),
                service_id: "web-1".to_string(),
                service_name: "web".to_string(),
                service_tags: vec!["v1".to_string()],
                check_type: String::new(),
                exposed_port: 0,
                interval: None,
                timeout: None,
                definition: None,
                create_index: 1,
                modify_index: 1,
            }],
        };

        let imported = convert_service_health(&sh, "peer-b");
        assert_eq!(imported.service_name, "web");
        assert_eq!(imported.service_id, "web-1");
        assert_eq!(imported.address, "10.0.0.5");
        assert_eq!(imported.port, 8080);
        assert_eq!(imported.tags, vec!["v1", "prod"]);
        assert_eq!(imported.meta.get("env").unwrap(), "prod");
        assert_eq!(imported.datacenter, "dc-remote");
        assert_eq!(imported.node, "remote-node");
        assert_eq!(imported.node_address, "10.0.0.5");
        assert_eq!(imported.checks.len(), 1);
        assert_eq!(imported.checks[0].check_id, "check-web-1");
        assert_eq!(imported.checks[0].status, "passing");
        assert_eq!(imported.checks[0].output, "ok");
        assert!(!imported.imported_at.is_empty());
    }

    #[tokio::test]
    async fn test_replicate_from_peer_unknown_peer_errors() {
        let service = ConsulPeeringService::new();
        let result = service.replicate_from_peer("no-such-peer").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[tokio::test]
    async fn test_replicate_from_peer_inactive_errors() {
        let service = ConsulPeeringService::new();
        let peering = Peering {
            id: uuid::Uuid::new_v4().to_string(),
            name: "pending-peer".to_string(),
            partition: String::new(),
            state: PeeringState::Pending,
            peer_id: uuid::Uuid::new_v4().to_string(),
            peer_server_name: String::new(),
            peer_server_addresses: vec!["127.0.0.1:8500".to_string()],
            peer_ca_pems: Vec::new(),
            meta: Default::default(),
            stream_status: Default::default(),
            create_index: 1,
            modify_index: 1,
            remote: PeeringRemoteInfo::default(),
            deleted_at: None,
        };
        service
            .peerings_for_test()
            .insert("pending-peer".to_string(), peering);

        let result = service.replicate_from_peer("pending-peer").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not active"));
    }

    #[tokio::test]
    async fn test_replicate_from_peer_no_addresses_errors() {
        let service = ConsulPeeringService::new();
        let peering = Peering {
            id: uuid::Uuid::new_v4().to_string(),
            name: "active-peer".to_string(),
            partition: String::new(),
            state: PeeringState::Active,
            peer_id: uuid::Uuid::new_v4().to_string(),
            peer_server_name: String::new(),
            peer_server_addresses: Vec::new(),
            peer_ca_pems: Vec::new(),
            meta: Default::default(),
            stream_status: Default::default(),
            create_index: 1,
            modify_index: 1,
            remote: PeeringRemoteInfo::default(),
            deleted_at: None,
        };
        service
            .peerings_for_test()
            .insert("active-peer".to_string(), peering);

        let result = service.replicate_from_peer("active-peer").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no server addresses"));
    }

    #[tokio::test]
    async fn test_replicate_from_peer_unreachable_returns_error() {
        let service = ConsulPeeringService::new();
        let peering = Peering {
            id: uuid::Uuid::new_v4().to_string(),
            name: "active-peer".to_string(),
            partition: String::new(),
            state: PeeringState::Active,
            peer_id: uuid::Uuid::new_v4().to_string(),
            peer_server_name: String::new(),
            // Port 1 is almost certainly not listening; connection refused is fast.
            peer_server_addresses: vec!["127.0.0.1:1".to_string()],
            peer_ca_pems: Vec::new(),
            meta: Default::default(),
            stream_status: Default::default(),
            create_index: 1,
            modify_index: 1,
            remote: PeeringRemoteInfo::default(),
            deleted_at: None,
        };
        service
            .peerings_for_test()
            .insert("active-peer".to_string(), peering);

        let result = service.replicate_from_peer("active-peer").await;
        assert!(result.is_err());
        // Should not have imported anything.
        assert!(service.get_imported_services("active-peer").is_empty());
    }

    /// Start a minimal HTTP server that returns canned Consul-style responses
    /// for catalog/services and health/service/:service. Returns the listening
    /// address as "host:port".
    fn start_mock_consul_server(
        services_response: &'static str,
        health_response: &'static str,
    ) -> String {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let addr_str = format!("127.0.0.1:{}", addr.port());

        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let mut stream = match stream {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let request = String::from_utf8_lossy(&buf);

                let body = if request.contains("/v1/catalog/services") {
                    services_response
                } else if request.contains("/v1/health/service/") {
                    health_response
                } else if request.contains("/v1/exported-services") {
                    "[]"
                } else {
                    "{}"
                };

                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            }
        });

        // Give the server a moment to start.
        std::thread::sleep(std::time::Duration::from_millis(50));
        addr_str
    }

    #[tokio::test]
    async fn test_replicate_from_peer_success_via_mock_server() {
        let services_json = r#"{"web":["v1"]}"#;
        let health_json = r#"[{"Node":{"ID":"n1","Node":"remote-node","Address":"10.0.0.9","Datacenter":"dc-remote","CreateIndex":1,"ModifyIndex":1},"Service":{"ID":"web-1","Service":"web","Tags":["v1"],"Port":9090,"Address":"10.0.0.9","Meta":{},"Weights":{"Passing":1,"Warning":1},"EnableTagOverride":false},"Checks":[{"Node":"remote-node","CheckID":"check-web-1","Name":"web health","Status":"passing","Notes":"","Output":"ok","ServiceID":"web-1","ServiceName":"web","ServiceTags":["v1"],"Type":"","ExposedPort":0,"CreateIndex":1,"ModifyIndex":1}]}]"#;

        let addr = start_mock_consul_server(services_json, health_json);

        let service = ConsulPeeringService::new();
        let peering = Peering {
            id: uuid::Uuid::new_v4().to_string(),
            name: "peer-mock".to_string(),
            partition: String::new(),
            state: PeeringState::Active,
            peer_id: uuid::Uuid::new_v4().to_string(),
            peer_server_name: String::new(),
            peer_server_addresses: vec![addr],
            peer_ca_pems: Vec::new(),
            meta: Default::default(),
            stream_status: Default::default(),
            create_index: 1,
            modify_index: 1,
            remote: PeeringRemoteInfo {
                partition: "default".to_string(),
                datacenter: "dc-remote".to_string(),
            },
            deleted_at: None,
        };
        service
            .peerings_for_test()
            .insert("peer-mock".to_string(), peering);

        let count = service.replicate_from_peer("peer-mock").await.unwrap();
        assert_eq!(count, 1);

        let instances = service.get_imported_services("peer-mock");
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].service_name, "web");
        assert_eq!(instances[0].service_id, "web-1");
        assert_eq!(instances[0].address, "10.0.0.9");
        assert_eq!(instances[0].port, 9090);
        assert_eq!(instances[0].node, "remote-node");
        assert_eq!(instances[0].datacenter, "dc-remote");
        assert_eq!(instances[0].checks[0].status, "passing");

        // stream_status.imported_services should be updated.
        let peering = service.get_peering("peer-mock").unwrap();
        assert_eq!(peering.stream_status.imported_services, vec!["web"]);
        assert!(peering.stream_status.last_receive.is_some());
    }
}
