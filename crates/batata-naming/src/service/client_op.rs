//! Client operation dispatch layer modeled on Nacos 3.x
//! `InstanceOperatorClientImpl` + `ClientOperationServiceProxy`.
//!
//! Every instance write funnels through [`ClientOperationService`]. A
//! proxy dispatches to:
//! - [`EphemeralClientOperationService`]: in-memory DashMap + Distro broadcast
//! - [`PersistentClientOperationService`]: Raft-replicated, with apply-back
//!   hook flowing into the same DashMap so reads see the change immediately.
//!
//! The split matches Nacos's CP/AP boundary: config-like state (persistent
//! services) goes through Raft for strong consistency; heartbeat-driven
//! state (ephemeral) goes through Distro for availability.

use std::sync::Arc;

use async_trait::async_trait;
use batata_api::naming::NamingServiceProvider;
use tracing::warn;

use crate::model::Instance;
use crate::service::NamingService;

/// Unified write interface for instance operations. Callers (gRPC handlers,
/// REST endpoints) should never touch `NamingService` directly — they go
/// through this trait so the ephemeral/persistent split is honored.
#[async_trait]
pub trait ClientOperationService: Send + Sync {
    async fn register_instance(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instance: Instance,
    ) -> bool;

    async fn deregister_instance(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instance: &Instance,
    ) -> bool;
}

/// AP path: DashMap write + Distro broadcast. Distro sync is fire-and-forget
/// so register latency stays low; the transport layer handles retries.
pub struct EphemeralClientOperationService {
    naming: Arc<NamingService>,
    distro: Option<Arc<batata_core::service::distro::DistroProtocol>>,
}

impl EphemeralClientOperationService {
    pub fn new(
        naming: Arc<NamingService>,
        distro: Option<Arc<batata_core::service::distro::DistroProtocol>>,
    ) -> Self {
        Self { naming, distro }
    }

    fn spawn_distro_sync(&self, namespace: &str, group_name: &str, service_name: &str) {
        let Some(distro) = self.distro.clone() else {
            return;
        };
        let service_key = crate::service::build_service_key(namespace, group_name, service_name);
        tokio::spawn(async move {
            distro
                .sync_data(
                    batata_core::service::distro::DistroDataType::NamingInstance,
                    &service_key,
                )
                .await;
        });
    }
}

#[async_trait]
impl ClientOperationService for EphemeralClientOperationService {
    async fn register_instance(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        mut instance: Instance,
    ) -> bool {
        instance.ephemeral = true;
        let ok = self
            .naming
            .register_instance(namespace, group_name, service_name, instance);
        if ok {
            self.spawn_distro_sync(namespace, group_name, service_name);
        }
        ok
    }

    async fn deregister_instance(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instance: &Instance,
    ) -> bool {
        let ok = self
            .naming
            .deregister_instance(namespace, group_name, service_name, instance);
        if ok {
            self.spawn_distro_sync(namespace, group_name, service_name);
        }
        ok
    }
}

/// CP path: write through Raft. The state machine apply hook (installed in
/// `main.rs`) flows the committed change back into `NamingService` on every
/// node, so reads on followers see the instance without waiting for their
/// own client_write.
///
/// When `raft_node` is `None` (single-node dev mode without Raft), the
/// service falls back to direct DashMap writes to preserve dev ergonomics.
/// In that mode, persistent instances do not survive process restart — a
/// WARN log is emitted at startup so the operator knows.
pub struct PersistentClientOperationService {
    naming: Arc<NamingService>,
    raft_node: Option<Arc<batata_consistency::raft::RaftNode>>,
}

impl PersistentClientOperationService {
    pub fn new(
        naming: Arc<NamingService>,
        raft_node: Option<Arc<batata_consistency::raft::RaftNode>>,
    ) -> Self {
        if raft_node.is_none() {
            warn!(
                "PersistentClientOperationService constructed without Raft node — \
                 persistent instances will NOT survive restart (dev mode only)"
            );
        }
        Self { naming, raft_node }
    }

    fn build_instance_id(instance: &Instance) -> String {
        let cluster = if instance.cluster_name.is_empty() {
            "DEFAULT"
        } else {
            &instance.cluster_name
        };
        format!("{}#{}#{}", instance.ip, instance.port, cluster)
    }
}

#[async_trait]
impl ClientOperationService for PersistentClientOperationService {
    async fn register_instance(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        mut instance: Instance,
    ) -> bool {
        instance.ephemeral = false;
        let Some(ref raft) = self.raft_node else {
            // Dev-mode fallback: direct DashMap write. No Raft → no durability.
            return self
                .naming
                .register_instance(namespace, group_name, service_name, instance);
        };
        let instance_id = Self::build_instance_id(&instance);
        let metadata_json =
            serde_json::to_string(&instance.metadata).unwrap_or_else(|_| "{}".to_string());
        let req = batata_consistency::raft::RaftRequest::PersistentInstanceRegister(Box::new(
            batata_consistency::raft::request::PersistentInstanceRegisterPayload {
                namespace_id: namespace.to_string(),
                group_name: group_name.to_string(),
                service_name: service_name.to_string(),
                instance_id,
                ip: instance.ip.clone(),
                port: instance.port as u16,
                weight: instance.weight,
                healthy: instance.healthy,
                enabled: instance.enabled,
                metadata: metadata_json,
                cluster_name: if instance.cluster_name.is_empty() {
                    "DEFAULT".to_string()
                } else {
                    instance.cluster_name.clone()
                },
            },
        ));
        match raft.write(req).await {
            Ok(resp) => resp.success,
            Err(e) => {
                warn!("Raft write PersistentInstanceRegister failed: {}", e);
                false
            }
        }
    }

    async fn deregister_instance(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instance: &Instance,
    ) -> bool {
        let Some(ref raft) = self.raft_node else {
            return self
                .naming
                .deregister_instance(namespace, group_name, service_name, instance);
        };
        let instance_id = Self::build_instance_id(instance);
        let req = batata_consistency::raft::RaftRequest::PersistentInstanceDeregister {
            namespace_id: namespace.to_string(),
            group_name: group_name.to_string(),
            service_name: service_name.to_string(),
            instance_id,
        };
        match raft.write(req).await {
            Ok(resp) => resp.success,
            Err(e) => {
                warn!("Raft write PersistentInstanceDeregister failed: {}", e);
                false
            }
        }
    }
}

/// Helper for REST handlers that have a `NamingServiceProvider` (from
/// `web::Data<Arc<dyn NamingServiceProvider>>`) and an optional
/// `Arc<RaftNode>` from `AppState`. Dispatches by `instance.ephemeral`
/// without requiring the REST layer to construct a proxy.
///
/// REST endpoints should use this instead of calling
/// `naming_service.register_instance()` directly, so persistent instances
/// go through Raft.
pub async fn register_instance_dispatch(
    naming_service: &Arc<dyn NamingServiceProvider>,
    raft_node: Option<&Arc<batata_consistency::raft::RaftNode>>,
    namespace: &str,
    group_name: &str,
    service_name: &str,
    instance: Instance,
) -> bool {
    if instance.ephemeral {
        return naming_service.register_instance(namespace, group_name, service_name, instance);
    }
    let Some(raft) = raft_node else {
        // Dev-mode fallback without Raft.
        return naming_service.register_instance(namespace, group_name, service_name, instance);
    };
    let cluster = if instance.cluster_name.is_empty() {
        "DEFAULT"
    } else {
        &instance.cluster_name
    };
    let instance_id = format!("{}#{}#{}", instance.ip, instance.port, cluster);
    let metadata_json =
        serde_json::to_string(&instance.metadata).unwrap_or_else(|_| "{}".to_string());
    let req = batata_consistency::raft::RaftRequest::PersistentInstanceRegister(Box::new(
        batata_consistency::raft::request::PersistentInstanceRegisterPayload {
            namespace_id: namespace.to_string(),
            group_name: group_name.to_string(),
            service_name: service_name.to_string(),
            instance_id,
            ip: instance.ip.clone(),
            port: instance.port as u16,
            weight: instance.weight,
            healthy: instance.healthy,
            enabled: instance.enabled,
            metadata: metadata_json,
            cluster_name: cluster.to_string(),
        },
    ));
    match raft.write(req).await {
        Ok(resp) => resp.success,
        Err(e) => {
            warn!("Raft write PersistentInstanceRegister failed (REST): {}", e);
            false
        }
    }
}

/// Companion helper for deregister — same dispatch rules.
pub async fn deregister_instance_dispatch(
    naming_service: &Arc<dyn NamingServiceProvider>,
    raft_node: Option<&Arc<batata_consistency::raft::RaftNode>>,
    namespace: &str,
    group_name: &str,
    service_name: &str,
    instance: &Instance,
) -> bool {
    if instance.ephemeral {
        return naming_service.deregister_instance(namespace, group_name, service_name, instance);
    }
    let Some(raft) = raft_node else {
        return naming_service.deregister_instance(namespace, group_name, service_name, instance);
    };
    let cluster = if instance.cluster_name.is_empty() {
        "DEFAULT"
    } else {
        &instance.cluster_name
    };
    let instance_id = format!("{}#{}#{}", instance.ip, instance.port, cluster);
    let req = batata_consistency::raft::RaftRequest::PersistentInstanceDeregister {
        namespace_id: namespace.to_string(),
        group_name: group_name.to_string(),
        service_name: service_name.to_string(),
        instance_id,
    };
    match raft.write(req).await {
        Ok(resp) => resp.success,
        Err(e) => {
            warn!(
                "Raft write PersistentInstanceDeregister failed (REST): {}",
                e
            );
            false
        }
    }
}

/// Dispatcher that routes every write to the correct underlying service
/// based on `instance.ephemeral`. Mirrors Nacos 3.x `ClientOperationServiceProxy`.
pub struct ClientOperationServiceProxy {
    ephemeral: Arc<EphemeralClientOperationService>,
    persistent: Arc<PersistentClientOperationService>,
}

impl ClientOperationServiceProxy {
    pub fn new(
        ephemeral: Arc<EphemeralClientOperationService>,
        persistent: Arc<PersistentClientOperationService>,
    ) -> Self {
        Self {
            ephemeral,
            persistent,
        }
    }

    pub fn ephemeral(&self) -> &Arc<EphemeralClientOperationService> {
        &self.ephemeral
    }

    pub fn persistent(&self) -> &Arc<PersistentClientOperationService> {
        &self.persistent
    }
}

#[async_trait]
impl ClientOperationService for ClientOperationServiceProxy {
    async fn register_instance(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instance: Instance,
    ) -> bool {
        if instance.ephemeral {
            self.ephemeral
                .register_instance(namespace, group_name, service_name, instance)
                .await
        } else {
            self.persistent
                .register_instance(namespace, group_name, service_name, instance)
                .await
        }
    }

    async fn deregister_instance(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instance: &Instance,
    ) -> bool {
        if instance.ephemeral {
            self.ephemeral
                .deregister_instance(namespace, group_name, service_name, instance)
                .await
        } else {
            self.persistent
                .deregister_instance(namespace, group_name, service_name, instance)
                .await
        }
    }
}
