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
    /// Registers the `register_instance` entry.
    async fn register_instance(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instance: Instance,
    ) -> bool;

    /// Removes the `deregister_instance` entry.
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
///
/// Registration is rejected when the **service** is persistent. Upstream Nacos
/// throws `NacosRuntimeException("Current service %s is persistent service,
/// can't register ephemeral instance.")` from
/// `EphemeralClientOperationServiceImpl#registerInstance`, after resolving the
/// service through `ServiceManager.getSingleton(service)` — see
/// `NamingService::resolve_service_ephemeral` for why the flag is fixed by the
/// first registration rather than by the caller.
///
/// On top of the upstream check there is a batata-specific guard on the
/// *instance* flag: batata's Distro publisher ships instances individually and
/// filters on `instance.ephemeral`, so admitting a `ephemeral=false` instance
/// into an already-ephemeral service — which upstream accepts — would leave a
/// locally-stored instance that no peer ever receives. Unreachable for
/// conforming clients (REST and gRPC both route by the instance flag), so it
/// only turns misuse into a visible error instead of silent divergence.
pub struct EphemeralClientOperationService {
    naming: Arc<NamingService>,
    distro: Option<Arc<batata_core::service::distro::DistroProtocol>>,
}

impl EphemeralClientOperationService {
    /// Creates a new instance.
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
        instance: Instance,
    ) -> bool {
        // batata-only hardening on top of the upstream check — see the
        // struct-level note for why the instance flag is inspected as well.
        //
        // It runs **before** `resolve_service_ephemeral` because that call fixes
        // the flag on first sight: resolving first would let a rejected write
        // brand the service persistent and then refuse every later ephemeral
        // registration for it.
        if !instance.ephemeral {
            warn!(
                "EphemeralClientOperationService refusing persistent instance \
                 {}:{} (service {}) — route it through PersistentClientOperationService",
                instance.ip, instance.port, service_name
            );
            return false;
        }
        // Nacos: `ServiceManager.getSingleton(service)` then
        // `if (!singleton.isEphemeral()) throw NacosRuntimeException(...)`.
        // batata reports failure through `false`, which the gRPC handler turns
        // into a FAILED InstanceResponse.
        //
        // The `Service` handed to `getSingleton` is built by the *route*, not
        // from the instance: `InstanceRequestHandler` uses
        // `Service.newService(namespace, group, name, true)`, so the AP path
        // always seeds/resolves `true` regardless of what the caller sent in
        // `instance.ephemeral`.
        if !self
            .naming
            .resolve_service_ephemeral(namespace, group_name, service_name, true)
        {
            warn!(
                "EphemeralClientOperationService refusing instance {}:{} — service {} is \
                 persistent, route it through PersistentClientOperationService",
                instance.ip, instance.port, service_name
            );
            return false;
        }
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
    /// Creates a new instance.
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
        instance: Instance,
    ) -> bool {
        // Same batata-only hardening as on the AP path: never let an ephemeral
        // instance slip into the CP path. It runs before the singleton is
        // resolved for the same reason as there — see the AP-path note.
        if instance.ephemeral {
            warn!(
                "PersistentClientOperationService refusing ephemeral instance \
                 {}:{} (service {}) — route it through EphemeralClientOperationService",
                instance.ip, instance.port, service_name
            );
            return false;
        }
        // Mirror of the AP-path gate:
        // `PersistentClientOperationServiceImpl#registerInstance` resolves the
        // singleton and throws "Current service %s is ephemeral service, can't
        // register persistent instance." when it is ephemeral. As on the AP
        // path the `Service` is built by the route
        // (`Service.newService(..., false)`), so the flag resolved here is the
        // route's own.
        if self
            .naming
            .resolve_service_ephemeral(namespace, group_name, service_name, false)
        {
            warn!(
                "PersistentClientOperationService refusing instance {}:{} — service {} is \
                 ephemeral, route it through EphemeralClientOperationService",
                instance.ip, instance.port, service_name
            );
            return false;
        }
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
                now_ms: chrono::Utc::now().timestamp_millis(),
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

/// Effective ephemeral flag for an **update** request.
///
/// Nacos `InstanceOperatorClientImpl#updateInstance` only rewrites instance
/// metadata — it never moves a service between the AP and CP paths. batata
/// implements an update as a re-registration, so an `ephemeral` field the
/// update form omits must inherit the stored instance's flag (falling back to
/// the resolved service flag) rather than defaulting to `true`; otherwise
/// updating a persistent instance would flip it onto the Distro path and drop
/// its Raft durability.
pub fn effective_update_ephemeral(
    naming_service: &Arc<dyn NamingServiceProvider>,
    namespace: &str,
    group_name: &str,
    service_name: &str,
    ip: &str,
    port: i32,
    cluster_name: &str,
    requested: Option<bool>,
) -> bool {
    if let Some(flag) = requested {
        return flag;
    }
    let instance_key = crate::service::build_instance_key_parts(ip, port, cluster_name);
    naming_service
        .get_instances_snapshot(namespace, group_name, service_name, "", false)
        .into_iter()
        .find(|i| i.key() == instance_key)
        .map(|i| i.ephemeral)
        .or_else(|| naming_service.service_ephemeral(namespace, group_name, service_name))
        .unwrap_or(true)
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
    // Nacos `InstanceOperatorClientImpl#registerInstance` builds
    // `Service.newService(namespaceId, groupName, serviceName, instance.isEphemeral())`
    // and lets the proxy's chosen operation service enforce the
    // service-level flag. Resolve it here for the same effect.
    if instance.ephemeral
        && !naming_service.resolve_service_ephemeral(namespace, group_name, service_name, true)
    {
        warn!(
            "REST register refused: service {} is persistent, cannot register ephemeral instance",
            service_name
        );
        return false;
    }
    if !instance.ephemeral
        && naming_service.resolve_service_ephemeral(namespace, group_name, service_name, false)
    {
        warn!(
            "REST register refused: service {} is ephemeral, cannot register persistent instance",
            service_name
        );
        return false;
    }
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
            now_ms: chrono::Utc::now().timestamp_millis(),
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
    /// Creates a new instance.
    pub fn new(
        ephemeral: Arc<EphemeralClientOperationService>,
        persistent: Arc<PersistentClientOperationService>,
    ) -> Self {
        Self {
            ephemeral,
            persistent,
        }
    }

    /// Performs the `ephemeral` operation.
    pub fn ephemeral(&self) -> &Arc<EphemeralClientOperationService> {
        &self.ephemeral
    }

    /// Performs the `persistent` operation.
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
