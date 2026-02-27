// Member lookup implementations for cluster node discovery
// Provides different strategies for discovering cluster members

use std::{
    collections::HashSet,
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
    sync::Arc,
    time::Duration,
};

use dashmap::DashMap;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use batata_api::model::{Member, MemberBuilder};

use crate::model::Configuration;

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
    running: Arc<RwLock<bool>>,
}

impl FileMemberLookup {
    pub fn new(config: Configuration) -> Self {
        Self {
            config,
            members: Arc::new(DashMap::new()),
            cluster_conf_path: "conf/cluster.conf".to_string(),
            running: Arc::new(RwLock::new(false)),
        }
    }

    pub fn with_path(config: Configuration, path: &str) -> Self {
        Self {
            config,
            members: Arc::new(DashMap::new()),
            cluster_conf_path: path.to_string(),
            running: Arc::new(RwLock::new(false)),
        }
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
}

#[tonic::async_trait]
impl MemberLookup for FileMemberLookup {
    async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut running = self.running.write().await;
        if *running {
            return Ok(());
        }

        info!("Starting file-based member lookup");
        self.load_members();
        *running = true;

        Ok(())
    }

    async fn stop(&self) {
        let mut running = self.running.write().await;
        *running = false;
        info!("Stopped file-based member lookup");
    }

    fn get_members(&self) -> Vec<Member> {
        self.members.iter().map(|e| e.value().clone()).collect()
    }
}

/// Address server based member lookup
/// Fetches cluster members from a remote address server
pub struct AddressServerMemberLookup {
    config: Configuration,
    members: Arc<DashMap<String, Member>>,
    running: Arc<RwLock<bool>>,
    address_server_url: String,
    refresh_interval: Duration,
}

impl AddressServerMemberLookup {
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

        Self {
            config,
            members: Arc::new(DashMap::new()),
            running: Arc::new(RwLock::new(false)),
            address_server_url: url,
            refresh_interval: Duration::from_secs(30),
        }
    }

    /// Fetch member list from address server
    async fn fetch_members(&self) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()?;

        let response = client.get(&self.address_server_url).send().await?;

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
        let url = self.address_server_url.clone();
        let interval = self.refresh_interval;
        let default_port = self.config.server_main_port();

        tokio::spawn(async move {
            let client = match reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
            {
                Ok(c) => c,
                Err(e) => {
                    error!("Failed to create HTTP client: {}", e);
                    return;
                }
            };

            loop {
                {
                    let is_running = running.read().await;
                    if !*is_running {
                        break;
                    }
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
        let mut running = self.running.write().await;
        if *running {
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

        *running = true;
        drop(running);

        // Start background refresh
        self.start_refresh_task().await;

        Ok(())
    }

    async fn stop(&self) {
        let mut running = self.running.write().await;
        *running = false;
        info!("Stopped address server based member lookup");
    }

    fn get_members(&self) -> Vec<Member> {
        self.members.iter().map(|e| e.value().clone()).collect()
    }

    fn use_address_server(&self) -> bool {
        true
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
}
