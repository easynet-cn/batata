//! ConsulPlugin - Protocol adapter plugin for Consul API compatibility.
//!
//! Implements the `Plugin` and `ProtocolAdapterPlugin` traits, encapsulating
//! all Consul services, routes, and app_data configuration.

use std::sync::Arc;


use rocksdb::DB;

use batata_naming::InstanceCheckRegistry;
use batata_plugin::ProtocolAdapterPlugin;

use crate::acl::AclService;
use crate::agent::ConsulAgentService;
use crate::catalog::ConsulCatalogService;
use crate::config_entry::ConsulConfigEntryService;
use crate::connect::ConsulConnectService;
use crate::connect_ca::ConsulConnectCAService;
use crate::coordinate::ConsulCoordinateService;
use crate::event::ConsulEventService;
use crate::health::ConsulHealthService;
use crate::index_provider::{ConsulIndexProvider, ConsulTableIndex};
use crate::kv::ConsulKVService;
use crate::lock::{ConsulLockService, ConsulSemaphoreService};
use crate::model::ConsulDatacenterConfig;
use crate::operator::ConsulOperatorService;
use crate::peering::ConsulPeeringService;
use crate::query::ConsulQueryService;
use crate::raft::ConsulRaftNode;
use crate::session::ConsulSessionService;
use crate::snapshot::ConsulSnapshotService;

/// Consul compatibility protocol adapter plugin.
///
/// Holds all Consul service instances and provides actix-web configuration.
/// Supports three storage modes: in-memory, RocksDB standalone, and Raft-replicated.
#[derive(Clone)]
pub struct ConsulPlugin {
    pub naming_store: Arc<crate::naming_store::ConsulNamingStore>,
    pub agent: ConsulAgentService,
    pub health: ConsulHealthService,
    pub kv: ConsulKVService,
    pub catalog: ConsulCatalogService,
    pub acl: AclService,
    pub session: ConsulSessionService,
    pub event: ConsulEventService,
    pub query: ConsulQueryService,
    pub lock: ConsulLockService,
    pub semaphore: ConsulSemaphoreService,
    pub peering: Arc<ConsulPeeringService>,
    pub config_entry: ConsulConfigEntryService,
    pub connect: ConsulConnectService,
    pub connect_ca: ConsulConnectCAService,
    pub coordinate: ConsulCoordinateService,
    pub snapshot: ConsulSnapshotService,
    pub operator: ConsulOperatorService,
    pub namespace_service: crate::namespace::ConsulNamespaceService,
    pub dc_config: ConsulDatacenterConfig,
    pub index_provider: ConsulIndexProvider,
    enabled: bool,
}

impl ConsulPlugin {
    /// Creates Consul plugin with in-memory storage.
    ///
    /// If `naming_store` is provided, uses it. Otherwise creates a new one.
    pub fn new(
        naming_store: Option<Arc<crate::naming_store::ConsulNamingStore>>,
        registry: Arc<InstanceCheckRegistry>,
        acl_enabled: bool,
        dc_config: ConsulDatacenterConfig,
    ) -> Self {
        let index_provider = ConsulIndexProvider::new();
        let session = ConsulSessionService::new();
        let kv = ConsulKVService::new();
        let kv_arc = Arc::new(kv.clone());
        let session_arc = Arc::new(session.clone());
        let lock = ConsulLockService::new(kv_arc.clone(), session_arc.clone());
        let semaphore = ConsulSemaphoreService::new(kv_arc, session_arc);
        let naming_store =
            naming_store.unwrap_or_else(|| Arc::new(crate::naming_store::ConsulNamingStore::new()));

        Self {
            naming_store: naming_store.clone(),
            agent: ConsulAgentService::new(naming_store.clone(), registry.clone()),
            health: ConsulHealthService::new(registry),
            kv,
            catalog: ConsulCatalogService::with_datacenter(
                naming_store.clone(),
                dc_config.datacenter.clone(),
            )
            .with_index_provider(index_provider.clone()),
            namespace_service: crate::namespace::ConsulNamespaceService::new(index_provider.clone()),
            index_provider,
            acl: if acl_enabled {
                AclService::new()
            } else {
                AclService::disabled()
            },
            session,
            event: ConsulEventService::new(),
            query: ConsulQueryService::new(),
            lock,
            semaphore,
            peering: Arc::new(ConsulPeeringService::new()),
            config_entry: ConsulConfigEntryService::new(),
            connect: ConsulConnectService::new(),
            connect_ca: ConsulConnectCAService::new(),
            coordinate: ConsulCoordinateService::new(),
            snapshot: ConsulSnapshotService::new(),
            operator: ConsulOperatorService::new(),
            dc_config,
            enabled: true,
        }
    }

    /// Creates Consul plugin with RocksDB persistence.
    pub fn with_persistence(
        naming_store: Option<Arc<crate::naming_store::ConsulNamingStore>>,
        registry: Arc<InstanceCheckRegistry>,
        acl_enabled: bool,
        db: Arc<DB>,
        dc_config: ConsulDatacenterConfig,
    ) -> Self {
        let index_provider = ConsulIndexProvider::new();
        let session = ConsulSessionService::with_rocks(db.clone());
        let kv = ConsulKVService::with_rocks(db.clone());
        let kv_arc = Arc::new(kv.clone());
        let session_arc = Arc::new(session.clone());
        let lock = ConsulLockService::new(kv_arc.clone(), session_arc.clone());
        let semaphore = ConsulSemaphoreService::new(kv_arc, session_arc);
        let naming_store =
            naming_store.unwrap_or_else(|| Arc::new(crate::naming_store::ConsulNamingStore::new()));

        Self {
            naming_store: naming_store.clone(),
            agent: ConsulAgentService::new(naming_store.clone(), registry.clone()),
            health: ConsulHealthService::new(registry),
            kv,
            catalog: ConsulCatalogService::with_datacenter(
                naming_store.clone(),
                dc_config.datacenter.clone(),
            )
            .with_index_provider(index_provider.clone()),
            acl: if acl_enabled {
                AclService::with_rocks(db.clone())
            } else {
                AclService::disabled()
            },
            session,
            event: ConsulEventService::new(),
            query: ConsulQueryService::with_rocks(db.clone()),
            lock,
            semaphore,
            peering: Arc::new(ConsulPeeringService::with_rocks(
                db.clone(),
                dc_config.datacenter.clone(),
                dc_config.consul_port,
            )),
            config_entry: ConsulConfigEntryService::with_rocks(db.clone()),
            connect: ConsulConnectService::new(),
            connect_ca: ConsulConnectCAService::with_rocks(db.clone()),
            coordinate: ConsulCoordinateService::with_rocks(
                db.clone(),
                dc_config.datacenter.clone(),
            ),
            snapshot: ConsulSnapshotService::with_rocks(db.clone()),
            operator: ConsulOperatorService::with_rocks(db),
            namespace_service: crate::namespace::ConsulNamespaceService::new(index_provider.clone()),
            dc_config,
            index_provider,
            enabled: true,
        }
    }

    /// Creates Consul plugin with Raft-replicated RocksDB storage (cluster mode).
    pub fn with_consul_raft(
        naming_store: Option<Arc<crate::naming_store::ConsulNamingStore>>,
        registry: Arc<InstanceCheckRegistry>,
        acl_enabled: bool,
        db: Arc<DB>,
        consul_raft: Arc<ConsulRaftNode>,
        table_index: ConsulTableIndex,
        dc_config: ConsulDatacenterConfig,
    ) -> Self {
        let index_provider: ConsulIndexProvider = table_index;
        let session = ConsulSessionService::with_raft(db.clone(), consul_raft.clone());
        let kv = ConsulKVService::with_raft(db.clone(), consul_raft);
        let kv_arc = Arc::new(kv.clone());
        let session_arc = Arc::new(session.clone());
        let lock = ConsulLockService::new(kv_arc.clone(), session_arc.clone());
        let semaphore = ConsulSemaphoreService::new(kv_arc, session_arc);
        let naming_store =
            naming_store.unwrap_or_else(|| Arc::new(crate::naming_store::ConsulNamingStore::new()));

        Self {
            naming_store: naming_store.clone(),
            agent: ConsulAgentService::new(naming_store.clone(), registry.clone()),
            health: ConsulHealthService::new(registry),
            kv,
            catalog: ConsulCatalogService::with_datacenter(
                naming_store.clone(),
                dc_config.datacenter.clone(),
            )
            .with_index_provider(index_provider.clone()),
            acl: if acl_enabled {
                AclService::with_rocks(db.clone())
            } else {
                AclService::disabled()
            },
            session,
            event: ConsulEventService::new(),
            query: ConsulQueryService::with_rocks(db.clone()),
            lock,
            semaphore,
            peering: Arc::new(ConsulPeeringService::with_rocks(
                db.clone(),
                dc_config.datacenter.clone(),
                dc_config.consul_port,
            )),
            config_entry: ConsulConfigEntryService::with_rocks(db.clone()),
            connect: ConsulConnectService::new(),
            connect_ca: ConsulConnectCAService::with_rocks(db.clone()),
            coordinate: ConsulCoordinateService::with_rocks(
                db.clone(),
                dc_config.datacenter.clone(),
            ),
            snapshot: ConsulSnapshotService::with_rocks(db.clone()),
            operator: ConsulOperatorService::with_rocks(db),
            namespace_service: crate::namespace::ConsulNamespaceService::new(index_provider.clone()),
            dc_config,
            index_provider,
            enabled: true,
        }
    }

}

// Implement the unified ProtocolAdapterPlugin SPI
#[async_trait::async_trait]
impl ProtocolAdapterPlugin for ConsulPlugin {
    fn name(&self) -> &str {
        "consul-compatibility"
    }

    fn protocol(&self) -> &str {
        "consul"
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn default_port(&self) -> u16 {
        self.dc_config.consul_port
    }

    async fn init(&self) -> anyhow::Result<()> {
        tracing::info!(
            "Consul compatibility plugin initialized (dc={}, version={})",
            self.dc_config.datacenter,
            self.dc_config.consul_version
        );
        Ok(())
    }

    async fn shutdown(&self) -> anyhow::Result<()> {
        tracing::info!("Consul compatibility plugin shutting down");
        Ok(())
    }

    fn configure(&self, cfg: &mut actix_web::web::ServiceConfig) {
        cfg.app_data(actix_web::web::Data::from(self.naming_store.clone()))
            .app_data(actix_web::web::Data::new(self.agent.clone()))
            .app_data(actix_web::web::Data::new(self.health.clone()))
            .app_data(actix_web::web::Data::new(self.kv.clone()))
            .app_data(actix_web::web::Data::new(self.catalog.clone()))
            .app_data(actix_web::web::Data::new(self.acl.clone()))
            .app_data(actix_web::web::Data::new(self.session.clone()))
            .app_data(actix_web::web::Data::new(self.event.clone()))
            .app_data(actix_web::web::Data::new(self.query.clone()))
            .app_data(actix_web::web::Data::new(self.lock.clone()))
            .app_data(actix_web::web::Data::new(self.semaphore.clone()))
            .app_data(actix_web::web::Data::from(self.peering.clone()))
            .app_data(actix_web::web::Data::new(self.config_entry.clone()))
            .app_data(actix_web::web::Data::new(self.connect.clone()))
            .app_data(actix_web::web::Data::new(self.connect_ca.clone()))
            .app_data(actix_web::web::Data::new(self.coordinate.clone()))
            .app_data(actix_web::web::Data::new(self.snapshot.clone()))
            .app_data(actix_web::web::Data::new(self.operator.clone()))
            .app_data(actix_web::web::Data::new(self.namespace_service.clone()))
            .app_data(actix_web::web::Data::new(self.dc_config.clone()))
            .app_data(actix_web::web::Data::new(self.index_provider.clone()))
            .service(crate::route::routes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consul_plugin_trait_impl() {
        // Verify ProtocolAdapterPlugin is correctly implemented
        let plugin = create_test_plugin();
        assert_eq!(ProtocolAdapterPlugin::name(&plugin), "consul-compatibility");
        assert_eq!(plugin.protocol(), "consul");
        assert!(plugin.is_enabled());
        assert_eq!(plugin.priority(), 0);
    }

    #[tokio::test]
    async fn test_consul_plugin_lifecycle() {
        let plugin = create_test_plugin();
        assert_eq!(ProtocolAdapterPlugin::name(&plugin), "consul-compatibility");
        assert!(plugin.init().await.is_ok());
        assert!(plugin.shutdown().await.is_ok());
        assert_eq!(plugin.default_port(), 8500);
    }

    fn create_test_plugin() -> ConsulPlugin {
        let naming = Arc::new(batata_naming::service::NamingService::new());
        let registry = Arc::new(InstanceCheckRegistry::with_naming_service(naming));
        let dc_config = ConsulDatacenterConfig::new("dc1".to_string());
        ConsulPlugin::new(None, registry, false, dc_config)
    }
}
