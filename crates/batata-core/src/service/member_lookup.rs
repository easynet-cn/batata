// Member lookup implementations for cluster node discovery
// Provides different strategies for discovering cluster members

use std::{
    collections::HashSet,
    fs::File,
    future::Future,
    io::{BufRead, BufReader},
    path::Path,
    pin::Pin,
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use dashmap::DashMap;
use tracing::{debug, error, info, warn};

use batata_api::model::{Member, MemberBuilder};

use crate::model::Configuration;

/// Hook invoked when a lookup refreshes its member source at runtime.
///
/// Mirrors Nacos `AbstractMemberLookup.afterLookup()` which forwards to
/// `ServerMemberManager.memberChange()`. Without it a reload only updates the
/// lookup's private cache: the member manager keeps the startup snapshot, no
/// join/leave event is published, and consumers such as the distro mapper never
/// learn about the change.
pub type MembersChangedHook =
    Arc<dyn Fn(Vec<Member>) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

/// Trait for member lookup strategies
#[tonic::async_trait]
pub trait MemberLookup: Send + Sync {
    /// Start the member lookup service
    async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Stop the member lookup service
    async fn stop(&self);

    /// Get all discovered members
    fn get_members(&self) -> Vec<Member>;

    /// Check if using address server
    fn use_address_server(&self) -> bool {
        false
    }

    /// Install a hook invoked whenever the lookup re-reads its member source.
    ///
    /// The hook must be installed before [`MemberLookup::start`] so that reloads
    /// detected by the background watcher reach the member manager.
    async fn set_members_changed_hook(&self, _hook: MembersChangedHook) {}
}

/// Lookup type enumeration
#[derive(Clone, Debug, PartialEq, Default)]
pub enum LookupType {
    /// File-based member lookup using cluster.conf
    #[default]
    File,
    /// Address server based member lookup
    AddressServer,
}

impl From<&str> for LookupType {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "address-server" | "addressserver" => LookupType::AddressServer,
            _ => LookupType::File,
        }
    }
}

/// File-based member lookup
/// Reads cluster members from conf/cluster.conf file
pub struct FileMemberLookup {
    config: Configuration,
    members: Arc<DashMap<String, Member>>,
    cluster_conf_path: String,
    running: Arc<AtomicBool>,
    /// Invoked with a fresh member snapshot after every reload.
    hook: Arc<RwLock<Option<MembersChangedHook>>>,
    /// How often cluster.conf is checked for a new modification time.
    poll_interval: Duration,
}

impl FileMemberLookup {
    /// Creates a new instance.
    pub fn new(config: Configuration) -> Self {
        Self::with_path(config, "conf/cluster.conf")
    }

    /// With Path.
    pub fn with_path(config: Configuration, path: &str) -> Self {
        Self {
            config,
            members: Arc::new(DashMap::new()),
            cluster_conf_path: path.to_string(),
            running: Arc::new(AtomicBool::new(false)),
            hook: Arc::new(RwLock::new(None)),
            poll_interval: Duration::from_secs(10),
        }
    }

    /// Override the cluster.conf poll interval (mainly for tests, where the
    /// default 10s delay is far too slow).
    pub fn with_poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }

    /// Notify the installed hook with the current member snapshot.
    async fn notify_members_changed(
        members: &Arc<DashMap<String, Member>>,
        hook: &Arc<RwLock<Option<MembersChangedHook>>>,
    ) {
        let Some(hook) = hook.read().ok().and_then(|guard| guard.clone()) else {
            return;
        };
        let snapshot: Vec<Member> = members.iter().map(|e| e.value().clone()).collect();
        (hook)(snapshot).await;
    }

    /// Parse cluster.conf file and return member addresses
    fn parse_cluster_conf(&self) -> Vec<String> {
        let path = Path::new(&self.cluster_conf_path);
        if !path.exists() {
            warn!("Cluster config file not found: {}", self.cluster_conf_path);
            return vec![];
        }

        let file = match File::open(path) {
            Ok(f) => f,
            Err(e) => {
                error!("Failed to open cluster config file: {}", e);
                return vec![];
            }
        };

        let reader = BufReader::new(file);
        let mut addresses = Vec::new();

        for line in reader.lines().map_while(Result::ok) {
            let line = line.trim();
            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            addresses.push(line.to_string());
        }

        addresses
    }

    /// Parse member list from configuration
    fn parse_member_list(&self) -> Vec<String> {
        if let Ok(member_list) = self.config.config.get_string("batata.member.list") {
            member_list
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        } else {
            vec![]
        }
    }

    /// Parse a member address string into a Member
    /// Format: ip:port?raft_port=xxx or ip (default port 8848)
    fn parse_member(address: &str, default_port: u16) -> Option<Member> {
        let parts: Vec<&str> = address.split('?').collect();
        let addr_part = parts.first()?;

        let (ip, port) = if addr_part.contains(':') {
            let addr_parts: Vec<&str> = addr_part.split(':').collect();
            if addr_parts.len() != 2 {
                return None;
            }
            let ip = addr_parts[0].to_string();
            let port = addr_parts[1].parse::<u16>().ok()?;
            (ip, port)
        } else {
            (addr_part.to_string(), default_port)
        };

        let member = MemberBuilder::new(ip, port).build();

        // Parse query parameters (e.g., raft_port=xxx)
        if parts.len() > 1 {
            for param in parts[1].split('&') {
                let kv: Vec<&str> = param.split('=').collect();
                if kv.len() == 2 {
                    let key = kv[0].trim();
                    let value = kv[1].trim();
                    if key == "raft_port"
                        && let Ok(raft_port) = value.parse::<u16>()
                        && let Ok(mut extend_info) = member.extend_info.write()
                    {
                        extend_info.insert(
                            Member::RAFT_PORT.to_string(),
                            serde_json::Value::from(raft_port),
                        );
                    }
                }
            }
        }

        // Set default raft port if not specified
        let has_raft_port = member
            .extend_info
            .read()
            .map(|info| info.contains_key(Member::RAFT_PORT))
            .unwrap_or(false);

        if !has_raft_port && let Ok(mut extend_info) = member.extend_info.write() {
            extend_info.insert(
                Member::RAFT_PORT.to_string(),
                serde_json::Value::from(member.calculate_raft_port()),
            );
        }

        Some(member)
    }

    /// Load members from file and config
    fn load_members(&self) {
        let default_port = self.config.server_main_port();

        // First try member list from config
        let mut addresses = self.parse_member_list();

        // If empty, try cluster.conf file
        if addresses.is_empty() {
            addresses = self.parse_cluster_conf();
        }

        // Clear existing members
        self.members.clear();

        // Parse and add members
        for addr in addresses {
            if let Some(member) = Self::parse_member(&addr, default_port) {
                info!("Discovered cluster member: {}", member.address);
                self.members.insert(member.address.clone(), member);
            }
        }

        info!("Loaded {} cluster members", self.members.len());
    }

    /// Start a background task that periodically checks cluster.conf for changes.
    /// Reloads members when the file's modification time changes.
    fn start_file_watch_task(&self) {
        let members = self.members.clone();
        let running = self.running.clone();
        let conf_path = self.cluster_conf_path.clone();
        let config = self.config.clone();
        let hook = self.hook.clone();
        let poll_interval = self.poll_interval;
        // Baseline is taken here, not inside the spawned task: the task body
        // only starts running when the runtime first polls it, so sampling
        // lazily would swallow any edit made between `start()` and the first
        // poll.
        let initial_modified = std::fs::metadata(&self.cluster_conf_path)
            .and_then(|m| m.modified())
            .ok();

        tokio::spawn(async move {
            let check_interval = poll_interval;
            let mut last_modified = initial_modified;

            loop {
                tokio::time::sleep(check_interval).await;

                if !running.load(Ordering::Relaxed) {
                    break;
                }

                let current_modified = std::fs::metadata(&conf_path)
                    .and_then(|m| m.modified())
                    .ok();

                if current_modified != last_modified {
                    info!(
                        "Detected change in {}, reloading cluster members",
                        conf_path
                    );
                    last_modified = current_modified;

                    // Re-read the file and update members
                    let path = Path::new(&conf_path);
                    if let Ok(file) = File::open(path) {
                        let reader = BufReader::new(file);
                        let default_port = config.server_main_port();
                        let mut new_addresses = HashSet::new();

                        for line in reader.lines().map_while(Result::ok) {
                            let line = line.trim().to_string();
                            if line.is_empty() || line.starts_with('#') {
                                continue;
                            }
                            if let Some(member) =
                                FileMemberLookup::parse_member(&line, default_port)
                            {
                                new_addresses.insert(member.address.clone());
                                if !members.contains_key(&member.address) {
                                    info!("New cluster member discovered: {}", member.address);
                                    members.insert(member.address.clone(), member);
                                }
                            }
                        }

                        // Remove members no longer in file
                        let to_remove: Vec<String> = members
                            .iter()
                            .filter(|e| !new_addresses.contains(e.key()))
                            .map(|e| e.key().clone())
                            .collect();
                        for addr in to_remove {
                            info!("Removing cluster member: {}", addr);
                            members.remove(&addr);
                        }

                        info!("Reloaded {} cluster members", members.len());

                        // Forward the snapshot so the member manager (and its
                        // join/leave listeners, e.g. the distro mapper) sees
                        // the change instead of just this lookup's cache.
                        Self::notify_members_changed(&members, &hook).await;
                    }
                }
            }
        });
    }
}

#[tonic::async_trait]
impl MemberLookup for FileMemberLookup {
    async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.running.load(Ordering::Relaxed) {
            return Ok(());
        }

        info!("Starting file-based member lookup");
        self.load_members();
        self.running.store(true, Ordering::Relaxed);

        // Start background file watcher for hot-reload
        self.start_file_watch_task();

        Ok(())
    }

    async fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
        info!("Stopped file-based member lookup");
    }

    fn get_members(&self) -> Vec<Member> {
        self.members.iter().map(|e| e.value().clone()).collect()
    }

    async fn set_members_changed_hook(&self, hook: MembersChangedHook) {
        if let Ok(mut guard) = self.hook.write() {
            *guard = Some(hook);
        }
    }
}

/// Address server based member lookup
/// Fetches cluster members from a remote address server
pub struct AddressServerMemberLookup {
    config: Configuration,
    members: Arc<DashMap<String, Member>>,
    running: Arc<AtomicBool>,
    client: reqwest::Client,
    address_server_url: String,
    refresh_interval: Duration,
    /// Invoked with a fresh member snapshot after every successful poll.
    hook: Arc<RwLock<Option<MembersChangedHook>>>,
}

impl AddressServerMemberLookup {
    /// Creates a new instance.
    pub fn new(config: Configuration) -> Self {
        let domain = config
            .config
            .get_string("address.server.domain")
            .unwrap_or_else(|_| "jmenv.tbsite.net".to_string());
        let port = config.config.get_int("address.server.port").unwrap_or(8080) as u16;
        let path = config
            .config
            .get_string("address.server.url")
            .unwrap_or_else(|_| "/nacos/serverlist".to_string());

        let url = format!("http://{}:{}{}", domain, port, path);

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_default();

        Self {
            config,
            members: Arc::new(DashMap::new()),
            running: Arc::new(AtomicBool::new(false)),
            client,
            address_server_url: url,
            refresh_interval: Duration::from_secs(30),
            hook: Arc::new(RwLock::new(None)),
        }
    }

    /// Fetch member list from address server
    async fn fetch_members(&self) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let response = self.client.get(&self.address_server_url).send().await?;

        if !response.status().is_success() {
            return Err(format!("Address server returned status: {}", response.status()).into());
        }

        let body = response.text().await?;
        let addresses: Vec<String> = body
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && !s.starts_with('#'))
            .collect();

        Ok(addresses)
    }

    /// Parse and update members from address list
    fn update_members(&self, addresses: Vec<String>) {
        let default_port = self.config.server_main_port();

        // Track current addresses
        let new_addresses: HashSet<String> = addresses
            .iter()
            .filter_map(|addr| {
                FileMemberLookup::parse_member(addr, default_port).map(|m| m.address)
            })
            .collect();

        // Remove members that are no longer in the list
        let to_remove: Vec<String> = self
            .members
            .iter()
            .filter(|e| !new_addresses.contains(e.key()))
            .map(|e| e.key().clone())
            .collect();

        for addr in to_remove {
            info!("Removing cluster member: {}", addr);
            self.members.remove(&addr);
        }

        // Add new members
        for addr in addresses {
            if let Some(member) = FileMemberLookup::parse_member(&addr, default_port)
                && !self.members.contains_key(&member.address)
            {
                info!("Discovered cluster member: {}", member.address);
                self.members.insert(member.address.clone(), member);
            }
        }
    }

    /// Start background refresh task
    async fn start_refresh_task(&self) {
        let members = self.members.clone();
        let running = self.running.clone();
        let client = self.client.clone();
        let url = self.address_server_url.clone();
        let interval = self.refresh_interval;
        let default_port = self.config.server_main_port();
        let hook = self.hook.clone();

        tokio::spawn(async move {
            loop {
                if !running.load(Ordering::Relaxed) {
                    break;
                }

                tokio::time::sleep(interval).await;

                match client.get(&url).send().await {
                    Ok(response) if response.status().is_success() => {
                        if let Ok(body) = response.text().await {
                            let addresses: Vec<String> = body
                                .lines()
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty() && !s.starts_with('#'))
                                .collect();

                            // Update members
                            let new_addresses: HashSet<String> = addresses
                                .iter()
                                .filter_map(|addr| {
                                    FileMemberLookup::parse_member(addr, default_port)
                                        .map(|m| m.address)
                                })
                                .collect();

                            // Remove old members
                            let to_remove: Vec<String> = members
                                .iter()
                                .filter(|e| !new_addresses.contains(e.key()))
                                .map(|e| e.key().clone())
                                .collect();

                            for addr in to_remove {
                                debug!("Removing cluster member: {}", addr);
                                members.remove(&addr);
                            }

                            // Add new members
                            for addr in addresses {
                                if let Some(member) =
                                    FileMemberLookup::parse_member(&addr, default_port)
                                    && !members.contains_key(&member.address)
                                {
                                    debug!("Discovered cluster member: {}", member.address);
                                    members.insert(member.address.clone(), member);
                                }
                            }

                            // Forward the snapshot so the member manager sees
                            // membership changes, not just this lookup's cache.
                            FileMemberLookup::notify_members_changed(&members, &hook).await;
                        }
                    }
                    Ok(response) => {
                        warn!(
                            "Address server returned non-success status: {}",
                            response.status()
                        );
                    }
                    Err(e) => {
                        warn!("Failed to fetch from address server: {}", e);
                    }
                }
            }
        });
    }
}

#[tonic::async_trait]
impl MemberLookup for AddressServerMemberLookup {
    async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.running.load(Ordering::Relaxed) {
            return Ok(());
        }

        info!(
            "Starting address server based member lookup, url: {}",
            self.address_server_url
        );

        // Initial fetch
        let max_retries = self
            .config
            .config
            .get_int("batata.core.address-server.retry")
            .unwrap_or(5) as u32;

        let mut last_error = None;
        for i in 0..max_retries {
            match self.fetch_members().await {
                Ok(addresses) => {
                    self.update_members(addresses);
                    break;
                }
                Err(e) => {
                    warn!(
                        "Failed to fetch members from address server (attempt {}/{}): {}",
                        i + 1,
                        max_retries,
                        e
                    );
                    last_error = Some(e);
                    if i < max_retries - 1 {
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    }
                }
            }
        }

        if self.members.is_empty()
            && let Some(e) = last_error
        {
            warn!("Failed to fetch initial members from address server: {}", e);
        }

        self.running.store(true, Ordering::Relaxed);

        // Start background refresh
        self.start_refresh_task().await;

        Ok(())
    }

    async fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
        info!("Stopped address server based member lookup");
    }

    fn get_members(&self) -> Vec<Member> {
        self.members.iter().map(|e| e.value().clone()).collect()
    }

    fn use_address_server(&self) -> bool {
        true
    }

    async fn set_members_changed_hook(&self, hook: MembersChangedHook) {
        if let Ok(mut guard) = self.hook.write() {
            *guard = Some(hook);
        }
    }
}

/// Create a member lookup instance based on configuration
pub fn create_member_lookup(config: &Configuration) -> Box<dyn MemberLookup> {
    let lookup_type = config
        .config
        .get_string("batata.core.member.lookup.type")
        .map(|s| LookupType::from(s.as_str()))
        .unwrap_or(LookupType::File);

    match lookup_type {
        LookupType::AddressServer => {
            info!("Using address server based member lookup");
            Box::new(AddressServerMemberLookup::new(config.clone()))
        }
        LookupType::File => {
            info!("Using file based member lookup");
            Box::new(FileMemberLookup::new(config.clone()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_member_with_port() {
        let member = FileMemberLookup::parse_member("192.168.1.1:8848", 8848).unwrap();
        assert_eq!(member.ip, "192.168.1.1");
        assert_eq!(member.port, 8848);
    }

    #[test]
    fn test_parse_member_without_port() {
        let member = FileMemberLookup::parse_member("192.168.1.1", 8848).unwrap();
        assert_eq!(member.ip, "192.168.1.1");
        assert_eq!(member.port, 8848);
    }

    #[test]
    fn test_parse_member_with_raft_port() {
        let member =
            FileMemberLookup::parse_member("192.168.1.1:8848?raft_port=7848", 8848).unwrap();
        assert_eq!(member.ip, "192.168.1.1");
        assert_eq!(member.port, 8848);

        let raft_port = member
            .extend_info
            .read()
            .unwrap()
            .get(Member::RAFT_PORT)
            .unwrap()
            .as_u64()
            .unwrap();
        assert_eq!(raft_port, 7848);
    }

    #[test]
    fn test_lookup_type_from_str() {
        assert_eq!(LookupType::from("file"), LookupType::File);
        assert_eq!(
            LookupType::from("address-server"),
            LookupType::AddressServer
        );
        assert_eq!(LookupType::from("AddressServer"), LookupType::AddressServer);
        assert_eq!(LookupType::from("unknown"), LookupType::File);
    }

    /// Editing cluster.conf at runtime must reach the installed hook.
    ///
    /// Nacos gets this via `FileConfigMemberLookup`'s watcher calling
    /// `afterLookup()` -> `memberManager.memberChange()`; without the hook the
    /// reload only refreshes the lookup's private map and the member manager
    /// keeps the boot-time snapshot forever.
    #[tokio::test]
    async fn test_file_lookup_notifies_hook_on_conf_change() {
        let dir = std::env::temp_dir().join(format!("batata-cluster-conf-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("cluster.conf");
        std::fs::write(&path, "10.0.0.1:8848\n").unwrap();

        let config = config::Config::builder()
            .set_default("batata.standalone", false)
            .unwrap()
            .set_default("batata.server.main.port", 8848)
            .unwrap()
            .set_default("batata.version", "1.0.0")
            .unwrap()
            .build()
            .unwrap();

        let lookup =
            FileMemberLookup::with_path(Configuration::from_config(config), path.to_str().unwrap())
                .with_poll_interval(Duration::from_millis(50));

        let (tx, mut rx) = tokio::sync::mpsc::channel(8);
        let hook: MembersChangedHook = Arc::new(move |members| {
            let tx = tx.clone();
            Box::pin(async move {
                let _ = tx.send(members.len()).await;
            })
        });
        lookup.set_members_changed_hook(hook).await;

        lookup.start().await.unwrap();
        assert_eq!(lookup.get_members().len(), 1);

        std::fs::write(&path, "10.0.0.1:8848\n10.0.0.2:8848\n10.0.0.3:8848\n").unwrap();

        let observed = tokio::time::timeout(Duration::from_secs(3), rx.recv())
            .await
            .ok()
            .flatten();

        lookup.stop().await;
        let _ = std::fs::remove_file(&path);

        assert_eq!(
            observed,
            Some(3),
            "hook must observe the reloaded members (lookup cache now holds {})",
            lookup.get_members().len()
        );
    }
}
