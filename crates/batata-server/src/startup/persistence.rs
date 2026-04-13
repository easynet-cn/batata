//! Persistence layer initialization.
//!
//! Handles creating the appropriate persistence backend (external DB, embedded RocksDB,
//! or distributed Raft) based on deployment configuration.

use std::sync::Arc;

use batata_common::ClusterManager;
use batata_consistency::RaftNode;
use batata_core::cluster::ServerMemberManager;
use batata_migration::{Migrator, MigratorTrait};
use batata_persistence::{DeployTopology, PersistenceService, StorageBackend};
use batata_server_common::model::config::Configuration;
use tracing::info;

/// Result of persistence layer initialization.
///
/// Contains all resources created during persistence setup, grouped together
/// to avoid an unwieldy tuple return.
pub struct PersistenceContext {
    pub database_connection: Option<sea_orm::DatabaseConnection>,
    pub server_member_manager: Option<Arc<ServerMemberManager>>,
    pub cluster_manager: Option<Arc<dyn ClusterManager>>,
    pub persistence: Option<Arc<dyn PersistenceService>>,
    /// Keep RocksDB handle alive for the duration of the process.
    pub _rocks_db: Option<Arc<rocksdb::DB>>,
    pub raft_node: Option<Arc<RaftNode>>,
}

impl PersistenceContext {
    /// Create an empty context for console-remote mode (no local persistence).
    pub fn empty() -> Self {
        Self {
            database_connection: None,
            server_member_manager: None,
            cluster_manager: None,
            persistence: None,
            _rocks_db: None,
            raft_node: None,
        }
    }
}

/// Initialize persistence layer based on storage backend and deploy topology.
///
/// Returns a `PersistenceContext` containing all created resources.
/// Initialize the persistence layer.
///
/// `extra_cf_names` are additional RocksDB column families required by plugins.
/// They are created alongside the core CFs when RocksDB is opened.
pub async fn init_persistence(
    configuration: &Configuration,
    extra_cf_names: &[String],
) -> Result<PersistenceContext, Box<dyn std::error::Error>> {
    let storage_backend = configuration.storage_backend();
    let deploy_topology = configuration.deploy_topology();
    info!(
        "Storage backend: {}, Deploy topology: {}",
        storage_backend, deploy_topology
    );

    match (storage_backend, deploy_topology) {
        (StorageBackend::ExternalDb, DeployTopology::Standalone) => {
            init_external_db_standalone(configuration).await
        }
        (StorageBackend::ExternalDb, DeployTopology::Cluster) => {
            init_external_db_cluster(configuration, extra_cf_names).await
        }
        (StorageBackend::Embedded, DeployTopology::Standalone) => {
            init_embedded_standalone(configuration).await
        }
        (StorageBackend::Embedded, DeployTopology::Cluster) => {
            init_embedded_cluster(configuration, extra_cf_names).await
        }
    }
}

/// ExternalDb + Standalone: all data goes directly to the shared database.
/// No Raft, no RocksDB.
async fn init_external_db_standalone(
    configuration: &Configuration,
) -> Result<PersistenceContext, Box<dyn std::error::Error>> {
    let db = configuration.database_connection().await?;

    if configuration.db_migration_enabled() {
        info!("Running database migrations...");
        Migrator::up(&db, None).await?;
        info!("Database migrations completed successfully");
    }

    let core_config = configuration.to_core_config();
    let smm = Arc::new(ServerMemberManager::new(&core_config));
    let cm: Arc<dyn ClusterManager> = smm.clone();
    let persist: Arc<dyn PersistenceService> = Arc::new(
        batata_persistence::ExternalDbPersistService::new(db.clone()),
    );

    Ok(PersistenceContext {
        database_connection: Some(db),
        server_member_manager: Some(smm),
        cluster_manager: Some(cm),
        persistence: Some(persist),
        _rocks_db: None,
        raft_node: None,
    })
}

async fn init_embedded_standalone(
    configuration: &Configuration,
) -> Result<PersistenceContext, Box<dyn std::error::Error>> {
    let rocksdb_dir = configuration.embedded_rocksdb_dir();
    info!(
        "Initializing standalone embedded storage at: {}",
        rocksdb_dir
    );

    let rocks_config = configuration.rocksdb_config();
    let shared_cache = rocks_config.create_shared_block_cache();
    let sm = batata_consistency::RocksStateMachine::with_full_options(
        &rocksdb_dir,
        Some(rocks_config.to_db_options()),
        Some(rocks_config.to_cf_options_with_cache(&shared_cache)),
        Some(rocks_config.to_history_cf_options(&shared_cache)),
        Some(rocks_config.to_write_options()),
        &[],
    )
    .await
    .map_err(|e| format!("Failed to initialize RocksDB state machine: {}", e))?;

    let rdb = sm.db();
    let persist: Arc<dyn PersistenceService> =
        Arc::new(batata_persistence::EmbeddedPersistService::from_state_machine(&sm));
    let core_config = configuration.to_core_config();
    let smm = Arc::new(ServerMemberManager::new(&core_config));
    let cm: Arc<dyn ClusterManager> = smm.clone();

    Ok(PersistenceContext {
        database_connection: None,
        server_member_manager: Some(smm),
        cluster_manager: Some(cm),
        persistence: Some(persist),
        _rocks_db: Some(rdb),
        raft_node: None,
    })
}

/// ExternalDb + Cluster: config/namespace/auth go directly to the shared
/// database, but persistent instances and locks still need Raft consensus.
/// This matches Nacos 3.x where CP protocol is always started in cluster
/// mode regardless of the storage backend.
async fn init_external_db_cluster(
    configuration: &Configuration,
    extra_cf_names: &[String],
) -> Result<PersistenceContext, Box<dyn std::error::Error>> {
    // DB connection + migrations (same as standalone)
    let db = configuration.database_connection().await?;

    if configuration.db_migration_enabled() {
        info!("Running database migrations...");
        Migrator::up(&db, None).await?;
        info!("Database migrations completed successfully");
    }

    // PersistenceService goes to DB for config/namespace/auth
    let persist: Arc<dyn PersistenceService> = Arc::new(
        batata_persistence::ExternalDbPersistService::new(db.clone()),
    );

    // Start RaftNode for persistent instances + locks
    let (raft_node, rdb) = create_raft_node(configuration, extra_cf_names).await?;
    let raft_node = Arc::new(raft_node);

    // Start the distributed lock expire scanner
    let _lock_expire_scanner =
        raft_node.start_lock_expire_scanner(std::time::Duration::from_secs(5));

    let core_config = configuration.to_core_config();
    let smm = Arc::new(ServerMemberManager::new(&core_config));
    let cm: Arc<dyn ClusterManager> = smm.clone();

    Ok(PersistenceContext {
        database_connection: Some(db),
        server_member_manager: Some(smm),
        cluster_manager: Some(cm),
        persistence: Some(persist),
        _rocks_db: Some(rdb),
        raft_node: Some(raft_node),
    })
}

/// Create a RaftNode + RocksDB instance. Shared by both embedded-cluster
/// and external-db-cluster modes.
async fn create_raft_node(
    configuration: &Configuration,
    extra_cf_names: &[String],
) -> Result<(batata_consistency::RaftNode, Arc<rocksdb::DB>), Box<dyn std::error::Error>> {
    let rocksdb_dir = configuration.embedded_rocksdb_dir();
    let main_port = configuration.server_main_port();

    // Determine this node's Raft address from cluster.conf.
    // All nodes must agree on the SAME set of member addresses
    // (using the SAME IPs from cluster.conf), so we find our own
    // entry by matching the port, and derive the raft port from it.
    let local_ip = batata_common::local_ip();
    let node_addr = {
        let cluster_addrs = configuration.cluster_member_addresses();
        let mut matched_ip = local_ip.clone();
        for addr_str in &cluster_addrs {
            let addr_part = addr_str.split('?').next().unwrap_or(addr_str);
            if let Some((ip, port_str)) = addr_part.rsplit_once(':')
                && let Ok(port) = port_str.parse::<u16>()
                && port == main_port
            {
                matched_ip = ip.to_string();
                break;
            }
        }
        let raft_port = main_port - batata_api::model::Member::DEFAULT_RAFT_OFFSET_PORT;
        format!("{}:{}", matched_ip, raft_port)
    };

    let node_id = batata_consistency::calculate_node_id(&node_addr);
    info!(
        "Initializing Raft node: node_id={}, addr={}, rocksdb_dir={}",
        node_id, node_addr, rocksdb_dir
    );

    let raft_config = batata_consistency::RaftConfig {
        election_timeout_ms: configuration.raft_election_timeout_ms(),
        heartbeat_interval_ms: configuration.raft_heartbeat_interval_ms(),
        rpc_request_timeout_ms: configuration.raft_rpc_timeout_ms(),
        snapshot_threshold: configuration.raft_snapshot_threshold(),
        snapshot_transfer_timeout_ms: configuration.raft_snapshot_transfer_timeout_ms(),
        forward_max_retries: configuration.raft_forward_max_retries(),
        forward_initial_delay_ms: configuration.raft_forward_initial_delay_ms(),
        grpc_tcp_keepalive_secs: configuration.raft_grpc_tcp_keepalive_secs(),
        grpc_tcp_nodelay: configuration.raft_grpc_tcp_nodelay(),
        grpc_http2_keepalive_interval_secs: configuration.raft_grpc_http2_keepalive_interval_secs(),
        grpc_http2_keepalive_timeout_secs: configuration.raft_grpc_http2_keepalive_timeout_secs(),
        data_dir: std::path::PathBuf::from(&rocksdb_dir),
        ..Default::default()
    };

    let rocks_config = configuration.rocksdb_config();
    let shared_cache = rocks_config.create_shared_block_cache();
    let (raft_node, rdb) = batata_consistency::RaftNode::new_with_full_options(
        node_id,
        node_addr,
        raft_config,
        Some(rocks_config.to_db_options()),
        Some(rocks_config.to_cf_options_with_cache(&shared_cache)),
        Some(rocks_config.to_history_cf_options(&shared_cache)),
        Some(rocks_config.to_write_options()),
        extra_cf_names,
    )
    .await
    .map_err(|e| format!("Failed to initialize Raft node: {}", e))?;

    Ok((raft_node, rdb))
}

async fn init_embedded_cluster(
    configuration: &Configuration,
    extra_cf_names: &[String],
) -> Result<PersistenceContext, Box<dyn std::error::Error>> {
    let (raft_node, rdb) = create_raft_node(configuration, extra_cf_names).await?;
    let raft_node = Arc::new(raft_node);

    // Start the distributed lock expire scanner. Runs on every node but only
    // issues replicated `LockExpire` writes when this node is the Raft
    // leader. Matches Nacos `LockExpireTask` cadence (5s default) for compatibility and
    // prevents indefinite growth of `CF_LOCKS` when clients forget to
    // release held locks.
    let _lock_expire_scanner =
        raft_node.start_lock_expire_scanner(std::time::Duration::from_secs(5));

    let reader = batata_consistency::RocksDbReader::new(rdb.clone());
    let cache_ttl = configuration.config_read_cache_ttl_secs();
    let cache_max = configuration.config_read_cache_max_entries();
    let persist: Arc<dyn PersistenceService> =
        Arc::new(batata_persistence::DistributedPersistService::with_cache(
            raft_node.clone(),
            reader,
            cache_ttl,
            cache_max,
        ));

    // Initialize single-node cluster in standalone mode
    if configuration.is_standalone() {
        info!("Standalone distributed mode: initializing single-node Raft cluster");
        let node_id = raft_node.node_id();
        let node_addr = raft_node.addr().to_string();
        let mut members = std::collections::BTreeMap::new();
        members.insert(node_id, openraft::BasicNode { addr: node_addr });
        if let Err(e) = raft_node.initialize(members).await {
            info!(
                "Raft cluster init result: {} (already initialized is OK)",
                e
            );
        }
    }

    let core_config = configuration.to_core_config();
    let smm = Arc::new(ServerMemberManager::new(&core_config));
    let cm: Arc<dyn ClusterManager> = smm.clone();

    Ok(PersistenceContext {
        database_connection: None,
        server_member_manager: Some(smm),
        cluster_manager: Some(cm),
        persistence: Some(persist),
        _rocks_db: Some(rdb),
        raft_node: Some(raft_node),
    })
}
