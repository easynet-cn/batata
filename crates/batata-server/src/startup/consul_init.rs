//! Consul service initialization.
//!
//! Handles creating Consul service adapters, Raft groups, RocksDB persistence,
//! TTL monitors, deregister monitors, and session cleanup tasks.

use std::sync::Arc;
use std::time::Duration;

use batata_naming::InstanceCheckRegistry;
use batata_naming::healthcheck::{deregister_monitor::DeregisterMonitor, ttl_monitor::TtlMonitor};
use batata_plugin_consul::constants::{
    CF_CONSUL_ACL, CF_CONSUL_CA_ROOTS, CF_CONSUL_CONFIG_ENTRIES, CF_CONSUL_COORDINATES,
    CF_CONSUL_EVENTS, CF_CONSUL_INTENTIONS, CF_CONSUL_KV, CF_CONSUL_OPERATOR, CF_CONSUL_PEERING,
    CF_CONSUL_QUERIES, CF_CONSUL_SESSIONS,
};
use batata_server_common::model::config::Configuration;
use rocksdb::ColumnFamilyDescriptor;
use tracing::{error, info};

use batata_plugin_consul::ConsulPlugin;

use super::GrpcServers;

/// Type alias for backward compatibility.
pub type ConsulServices = ConsulPlugin;

/// Configuration for Consul service initialization.
pub struct ConsulInitConfig {
    pub consul_enabled: bool,
    pub consul_acl_enabled: bool,
    pub consul_server_port: u16,
    pub consul_server_address: String,
    pub consul_register_self: bool,
    pub consul_data_dir: String,
    pub consul_dc_config: batata_plugin_consul::model::ConsulDatacenterConfig,
}

impl ConsulInitConfig {
    /// Build from application configuration.
    pub fn from_config(configuration: &Configuration) -> Self {
        Self {
            consul_enabled: configuration.consul_enabled(),
            consul_acl_enabled: configuration.consul_acl_enabled(),
            consul_server_port: configuration.consul_server_port(),
            consul_server_address: configuration.server_address(),
            consul_register_self: configuration.consul_register_self(),
            consul_data_dir: configuration.consul_data_dir(),
            consul_dc_config: batata_plugin_consul::model::ConsulDatacenterConfig::new(
                configuration.consul_datacenter(),
            )
            .with_primary(configuration.consul_primary_datacenter())
            .with_consul_version(configuration.consul_version())
            .with_batata_version(configuration.batata_version())
            .with_consul_port(configuration.consul_server_port()),
        }
    }
}

/// Initialize Consul services if enabled.
///
/// Returns `Some(ConsulServices)` if Consul is enabled, `None` otherwise.
/// Also starts background tasks (TTL monitor, deregister monitor, session cleanup).
pub async fn init_consul(
    consul_config: &ConsulInitConfig,
    configuration: &Configuration,
    grpc_servers: &GrpcServers,
    raft_node: Option<&Arc<batata_consistency::RaftNode>>,
    is_console_remote: bool,
) -> Result<Option<ConsulServices>, Box<dyn std::error::Error>> {
    if !consul_config.consul_enabled {
        info!("Consul compatibility server is disabled");
        return Ok(None);
    }

    let is_cluster = !configuration.is_standalone() && !is_console_remote;

    // Create Consul naming store and result handler BEFORE the registry.
    // This ensures the registry syncs health status to ConsulNamingStore,
    // not NamingService — keeping Consul data fully independent.
    let consul_naming_store = Arc::new(batata_plugin_consul::ConsulNamingStore::new());
    let consul_index_provider = Arc::new(batata_plugin_consul::ConsulIndexProvider::new());
    let consul_result_handler: Arc<dyn batata_plugin::HealthCheckResultHandler> = Arc::new(
        batata_plugin_consul::ConsulResultHandler::new(
            consul_naming_store.clone(),
            consul_index_provider.clone(),
        ),
    );
    let consul_registry = Arc::new(InstanceCheckRegistry::new(consul_result_handler));

    let services = if is_cluster {
        init_consul_cluster(
            consul_config,
            configuration,
            grpc_servers,
            raft_node,
            &consul_naming_store,
            &consul_registry,
        )
        .await
    } else if !is_console_remote {
        init_consul_standalone(
            consul_config,
            configuration,
            &consul_naming_store,
            &consul_registry,
        )
    } else {
        info!("Console remote mode: Consul services using in-memory storage");
        ConsulPlugin::new(
            Some(consul_naming_store.clone()),
            consul_registry.clone(),
            consul_config.consul_acl_enabled,
            consul_config.consul_dc_config.clone(),
        )
    };

    // Auto-register Consul service if enabled (only in non-remote mode)
    if consul_config.consul_register_self && !is_console_remote {
        info!("Auto-registering Consul service...");
        let agent = services.agent.clone();
        let dc = consul_config.consul_dc_config.datacenter.clone();
        let consul_server_port = consul_config.consul_server_port;
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(2)).await;
            if let Err(e) = agent.register_consul_service(consul_server_port, &dc).await {
                error!("Failed to auto-register Consul service: {}", e);
            }
        });
    }

    // Start background monitors
    start_consul_monitors(&consul_registry, grpc_servers, &services);

    Ok(Some(services))
}

/// Initialize Consul in cluster mode with a dedicated Raft group.
async fn init_consul_cluster(
    consul_config: &ConsulInitConfig,
    configuration: &Configuration,
    grpc_servers: &GrpcServers,
    raft_node: Option<&Arc<batata_consistency::RaftNode>>,
    consul_naming_store: &Arc<batata_plugin_consul::ConsulNamingStore>,
    consul_registry: &Arc<InstanceCheckRegistry>,
) -> ConsulServices {
    let consul_node_id = raft_node.map(|r| r.node_id()).unwrap_or(1);
    let consul_node_addr = configuration.server_address();

    match batata_plugin_consul::raft::ConsulRaftNode::new(
        consul_node_id,
        consul_node_addr.clone(),
        &consul_config.consul_data_dir,
    )
    .await
    {
        Ok((consul_raft_node, consul_db, consul_table_index)) => {
            let consul_raft = Arc::new(consul_raft_node);
            info!(
                "Consul Raft node created (id={}, dir={})",
                consul_node_id, consul_config.consul_data_dir
            );

            // Activate the gRPC service (already listening on port 9849)
            grpc_servers
                .consul_raft_grpc()
                .set_raft_node(consul_raft.clone())
                .await;

            // Initialize Consul Raft in background
            if let Some(nacos_raft) = raft_node {
                spawn_consul_raft_init(configuration, nacos_raft, &consul_raft);
            }

            ConsulPlugin::with_consul_raft(
                Some(consul_naming_store.clone()),
                consul_registry.clone(),
                consul_config.consul_acl_enabled,
                consul_db,
                consul_raft,
                consul_table_index,
                consul_config.consul_dc_config.clone(),
            )
        }
        Err(e) => {
            error!(
                "Failed to create Consul Raft: {}. Falling back to standalone.",
                e
            );
            init_consul_standalone(
                consul_config,
                configuration,
                consul_naming_store,
                consul_registry,
            )
        }
    }
}

/// Spawn background task to initialize Consul Raft cluster members.
///
/// Waits for all peer Consul Raft gRPC servers to become reachable before
/// calling initialize(), matching the Nacos Raft initialization pattern
/// (see cluster.rs). This prevents premature leader election when some
/// peers haven't bound their Consul Raft ports yet.
fn spawn_consul_raft_init(
    configuration: &Configuration,
    nacos_raft: &Arc<batata_consistency::RaftNode>,
    consul_raft: &Arc<batata_plugin_consul::raft::ConsulRaftNode>,
) {
    let nacos_metrics = nacos_raft.metrics();
    let local_raft_port = configuration.raft_port();
    let consul_raft_port = configuration.consul_raft_port();

    let members: std::collections::BTreeMap<u64, openraft::BasicNode> = nacos_metrics
        .membership_config
        .membership()
        .voter_ids()
        .map(|id| {
            let nacos_addr = nacos_metrics
                .membership_config
                .membership()
                .get_node(&id)
                .map(|n| n.addr.clone())
                .unwrap_or_default();

            let consul_addr = if let Some((host, port_str)) = nacos_addr.rsplit_once(':') {
                if let Ok(nacos_rp) = port_str.parse::<i32>() {
                    let diff = consul_raft_port as i32 - local_raft_port as i32;
                    let consul_rp = (nacos_rp + diff) as u16;
                    format!("{}:{}", host, consul_rp)
                } else {
                    format!("{}:{}", host, consul_raft_port)
                }
            } else {
                nacos_addr
            };
            (id, openraft::BasicNode { addr: consul_addr })
        })
        .collect();

    if !members.is_empty() {
        let consul_raft_bg = consul_raft.clone();
        let local_addr = format!("127.0.0.1:{}", consul_raft_port);
        let timeout_secs = configuration.raft_peer_connect_timeout_secs();
        let retry_interval_ms = configuration.raft_peer_connect_retry_interval_ms();
        info!("Consul Raft members: {:?}", members);
        tokio::spawn(async move {
            // Wait for all peer Consul Raft gRPC servers to become reachable
            let peer_addrs: Vec<String> = members
                .values()
                .filter(|n| n.addr != local_addr)
                .map(|n| n.addr.clone())
                .collect();

            if !peer_addrs.is_empty() {
                info!(
                    "Waiting for {} Consul Raft peer(s) to become reachable (timeout: {}s)...",
                    peer_addrs.len(),
                    timeout_secs
                );
                let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs);

                for addr in &peer_addrs {
                    loop {
                        match tokio::net::TcpStream::connect(addr).await {
                            Ok(_) => {
                                info!("Consul Raft peer {} is reachable", addr);
                                break;
                            }
                            Err(_) => {
                                if tokio::time::Instant::now() >= deadline {
                                    tracing::warn!(
                                        "Timeout waiting for Consul Raft peer {} - proceeding anyway",
                                        addr
                                    );
                                    break;
                                }
                                tokio::time::sleep(Duration::from_millis(retry_interval_ms)).await;
                            }
                        }
                    }
                }
                info!("All Consul Raft peers checked, proceeding with initialization");
            }

            if let Err(e) = consul_raft_bg.initialize(members).await {
                tracing::debug!("Consul Raft init: {} (may already be initialized)", e);
            } else {
                info!("Consul Raft cluster initialized");
            }
        });
    }
}

/// Initialize Consul in standalone mode with RocksDB persistence.
fn init_consul_standalone(
    consul_config: &ConsulInitConfig,
    configuration: &Configuration,
    consul_naming_store: &Arc<batata_plugin_consul::ConsulNamingStore>,
    consul_registry: &Arc<InstanceCheckRegistry>,
) -> ConsulServices {
    let consul_rocks_db = open_consul_rocks_db(
        &consul_config.consul_data_dir,
        &configuration.rocksdb_config(),
    );
    if let Some(db) = consul_rocks_db {
        info!("Consul services using RocksDB persistence");
        ConsulPlugin::with_persistence(
            Some(consul_naming_store.clone()),
            consul_registry.clone(),
            consul_config.consul_acl_enabled,
            db,
            consul_config.consul_dc_config.clone(),
        )
    } else {
        info!("Consul services using in-memory storage (no persistence)");
        ConsulPlugin::new(
            Some(consul_naming_store.clone()),
            consul_registry.clone(),
            consul_config.consul_acl_enabled,
            consul_config.consul_dc_config.clone(),
        )
    }
}

/// Start background Consul monitors (TTL, deregister, session cleanup).
fn start_consul_monitors(
    consul_registry: &Arc<InstanceCheckRegistry>,
    _grpc_servers: &GrpcServers,
    services: &ConsulServices,
) {
    // TTL monitor
    info!("Starting Consul TTL monitor...");
    let ttl_monitor = TtlMonitor::new(consul_registry.clone());
    tokio::spawn(async move {
        ttl_monitor.start().await;
    });

    // Deregister monitor
    info!("Starting Consul deregister monitor...");
    let deregister_monitor = DeregisterMonitor::new(consul_registry.clone(), 30);
    tokio::spawn(async move {
        deregister_monitor.start().await;
    });

    // Session TTL cleanup
    let session_svc = services.session.clone();
    let kv_svc = services.kv.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            let expired = session_svc.scan_expired_session_ids();
            if !expired.is_empty() {
                info!("Cleaning up {} expired Consul sessions", expired.len());
                for id in &expired {
                    kv_svc.release_session(id).await;
                }
                session_svc.cleanup_expired();
            }
        }
    });
}

/// Open an independent RocksDB for Consul KV/Session/ACL storage.
fn open_consul_rocks_db(
    data_dir: &str,
    rocks_config: &batata_server_common::model::config::RocksDbConfig,
) -> Option<Arc<rocksdb::DB>> {
    info!("Initializing Consul RocksDB persistence at: {}", data_dir);

    let db_opts = rocks_config.to_db_options();
    let cf_opts = rocks_config.to_cf_options();

    let consul_cfs = vec![
        ColumnFamilyDescriptor::new(CF_CONSUL_KV, cf_opts.clone()),
        ColumnFamilyDescriptor::new(CF_CONSUL_ACL, cf_opts.clone()),
        ColumnFamilyDescriptor::new(CF_CONSUL_SESSIONS, cf_opts.clone()),
        ColumnFamilyDescriptor::new(CF_CONSUL_QUERIES, cf_opts.clone()),
        ColumnFamilyDescriptor::new(CF_CONSUL_CONFIG_ENTRIES, cf_opts.clone()),
        ColumnFamilyDescriptor::new(CF_CONSUL_CA_ROOTS, cf_opts.clone()),
        ColumnFamilyDescriptor::new(CF_CONSUL_INTENTIONS, cf_opts.clone()),
        ColumnFamilyDescriptor::new(CF_CONSUL_COORDINATES, cf_opts.clone()),
        ColumnFamilyDescriptor::new(CF_CONSUL_PEERING, cf_opts.clone()),
        ColumnFamilyDescriptor::new(CF_CONSUL_OPERATOR, cf_opts.clone()),
        ColumnFamilyDescriptor::new(CF_CONSUL_EVENTS, cf_opts),
    ];

    match rocksdb::DB::open_cf_descriptors(&db_opts, data_dir, consul_cfs) {
        Ok(db) => {
            info!("Consul RocksDB initialized successfully");
            Some(Arc::new(db))
        }
        Err(e) => {
            error!(
                "Failed to initialize Consul RocksDB: {}, falling back to in-memory",
                e
            );
            None
        }
    }
}
