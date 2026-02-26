//! Multi-datacenter support module
//!
//! This module provides datacenter-aware cluster management, enabling:
//! - Datacenter topology discovery and management
//! - Local-first data synchronization
//! - Cross-datacenter replication strategies
//! - Locality-aware member selection

use std::collections::HashSet;
use std::sync::Arc;

use dashmap::DashMap;
use tracing::{debug, info, warn};

use batata_api::model::Member;

/// Configuration for datacenter settings
#[derive(Debug, Clone)]
pub struct DatacenterConfig {
    /// The local datacenter name
    pub local_datacenter: String,
    /// The local region name
    pub local_region: String,
    /// The local zone name
    pub local_zone: String,
    /// Priority for local-first sync (higher = prefer local)
    pub locality_weight: f64,
    /// Enable cross-datacenter replication
    pub cross_dc_replication_enabled: bool,
    /// Delay in seconds for cross-datacenter sync
    pub cross_dc_sync_delay_secs: u64,
    /// Number of replicas required per datacenter
    pub replication_factor: usize,
}

impl Default for DatacenterConfig {
    fn default() -> Self {
        Self {
            local_datacenter: Member::DEFAULT_DATACENTER.to_string(),
            local_region: Member::DEFAULT_REGION.to_string(),
            local_zone: Member::DEFAULT_ZONE.to_string(),
            locality_weight: 1.0,
            cross_dc_replication_enabled: true,
            cross_dc_sync_delay_secs: 1,
            replication_factor: 1,
        }
    }
}

impl DatacenterConfig {
    /// Create from environment variables
    pub fn from_env() -> Self {
        Self {
            local_datacenter: std::env::var("BATATA_DATACENTER")
                .unwrap_or_else(|_| Member::DEFAULT_DATACENTER.to_string()),
            local_region: std::env::var("BATATA_REGION")
                .unwrap_or_else(|_| Member::DEFAULT_REGION.to_string()),
            local_zone: std::env::var("BATATA_ZONE")
                .unwrap_or_else(|_| Member::DEFAULT_ZONE.to_string()),
            locality_weight: std::env::var("BATATA_LOCALITY_WEIGHT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1.0),
            cross_dc_replication_enabled: std::env::var("BATATA_CROSS_DC_REPLICATION")
                .map(|v| v.to_lowercase() == "true" || v == "1")
                .unwrap_or(true),
            cross_dc_sync_delay_secs: std::env::var("BATATA_CROSS_DC_SYNC_DELAY_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1),
            replication_factor: std::env::var("BATATA_REPLICATION_FACTOR")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1),
        }
    }
}

/// Information about a datacenter
#[derive(Debug, Clone)]
pub struct DatacenterInfo {
    /// Datacenter name
    pub name: String,
    /// Region this datacenter belongs to
    pub region: String,
    /// List of zones in this datacenter
    pub zones: HashSet<String>,
    /// Number of healthy members
    pub healthy_member_count: usize,
    /// Total number of members
    pub total_member_count: usize,
    /// Whether this is the local datacenter
    pub is_local: bool,
}

/// Manages datacenter topology and provides locality-aware operations
pub struct DatacenterManager {
    /// Local datacenter configuration
    config: DatacenterConfig,
    /// Members organized by datacenter
    members_by_dc: DashMap<String, Vec<Arc<Member>>>,
    /// Datacenter information cache
    datacenter_info: DashMap<String, DatacenterInfo>,
}

impl DatacenterManager {
    /// Create a new DatacenterManager with the given configuration
    pub fn new(config: DatacenterConfig) -> Self {
        info!(
            "Initializing DatacenterManager: local_dc={}, local_region={}, local_zone={}",
            config.local_datacenter, config.local_region, config.local_zone
        );
        Self {
            config,
            members_by_dc: DashMap::new(),
            datacenter_info: DashMap::new(),
        }
    }

    /// Get the local datacenter name
    pub fn local_datacenter(&self) -> &str {
        &self.config.local_datacenter
    }

    /// Get the local region name
    pub fn local_region(&self) -> &str {
        &self.config.local_region
    }

    /// Get the local zone name
    pub fn local_zone(&self) -> &str {
        &self.config.local_zone
    }

    /// Check if cross-datacenter replication is enabled
    pub fn is_cross_dc_replication_enabled(&self) -> bool {
        self.config.cross_dc_replication_enabled
    }

    /// Get the cross-datacenter sync delay
    pub fn cross_dc_sync_delay_secs(&self) -> u64 {
        self.config.cross_dc_sync_delay_secs
    }

    /// Get the replication factor
    pub fn replication_factor(&self) -> usize {
        self.config.replication_factor
    }

    /// Update members from the cluster
    pub fn update_members(&self, members: Vec<Arc<Member>>) {
        // Clear existing data
        self.members_by_dc.clear();
        self.datacenter_info.clear();

        // Group members by datacenter
        for member in members {
            let dc = member.datacenter();
            self.members_by_dc
                .entry(dc.clone())
                .or_default()
                .push(member.clone());
        }

        // Build datacenter info
        for entry in self.members_by_dc.iter() {
            let dc_name = entry.key().clone();
            let members = entry.value();

            let mut zones = HashSet::new();
            let mut healthy_count = 0;

            for member in members {
                zones.insert(member.zone());
                if member.is_healthy() {
                    healthy_count += 1;
                }
            }

            // Determine region (use first member's region)
            let region = members
                .first()
                .map(|m| m.region())
                .unwrap_or_else(|| Member::DEFAULT_REGION.to_string());

            let info = DatacenterInfo {
                name: dc_name.clone(),
                region,
                zones,
                healthy_member_count: healthy_count,
                total_member_count: members.len(),
                is_local: dc_name == self.config.local_datacenter,
            };

            self.datacenter_info.insert(dc_name, info);
        }

        debug!(
            "Updated datacenter topology: {} datacenters",
            self.datacenter_info.len()
        );
    }

    /// Get all known datacenters
    pub fn get_all_datacenters(&self) -> Vec<DatacenterInfo> {
        self.datacenter_info
            .iter()
            .map(|e| e.value().clone())
            .collect()
    }

    /// Get members in a specific datacenter
    pub fn get_members_by_datacenter(&self, datacenter: &str) -> Vec<Arc<Member>> {
        self.members_by_dc
            .get(datacenter)
            .map(|e| e.value().clone())
            .unwrap_or_default()
    }

    /// Get members in the local datacenter
    pub fn get_local_members(&self) -> Vec<Arc<Member>> {
        self.get_members_by_datacenter(&self.config.local_datacenter)
    }

    /// Get members in remote datacenters
    pub fn get_remote_members(&self) -> Vec<Arc<Member>> {
        let mut remote = Vec::new();
        for entry in self.members_by_dc.iter() {
            if entry.key() != &self.config.local_datacenter {
                remote.extend(entry.value().clone());
            }
        }
        remote
    }

    /// Get healthy members in a specific datacenter
    pub fn get_healthy_members_by_datacenter(&self, datacenter: &str) -> Vec<Arc<Member>> {
        self.get_members_by_datacenter(datacenter)
            .into_iter()
            .filter(|m| m.is_healthy())
            .collect()
    }

    /// Get all healthy local members
    pub fn get_healthy_local_members(&self) -> Vec<Arc<Member>> {
        self.get_healthy_members_by_datacenter(&self.config.local_datacenter)
    }

    /// Get all healthy remote members
    pub fn get_healthy_remote_members(&self) -> Vec<Arc<Member>> {
        self.get_remote_members()
            .into_iter()
            .filter(|m| m.is_healthy())
            .collect()
    }

    /// Select members for replication based on locality preference
    ///
    /// Returns members sorted by locality (local first, then remote)
    pub fn select_replication_targets(
        &self,
        exclude_self: Option<&str>,
        max_count: usize,
    ) -> Vec<Arc<Member>> {
        let mut local_members = self.get_healthy_local_members();
        let mut remote_members = self.get_healthy_remote_members();

        // Remove self from the list if specified
        if let Some(self_addr) = exclude_self {
            local_members.retain(|m| m.address != self_addr);
            remote_members.retain(|m| m.address != self_addr);
        }

        // Sort by locality weight (higher weight = higher priority)
        local_members.sort_by(|a, b| {
            b.locality_weight()
                .partial_cmp(&a.locality_weight())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        remote_members.sort_by(|a, b| {
            b.locality_weight()
                .partial_cmp(&a.locality_weight())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Combine local first, then remote
        let mut targets = Vec::with_capacity(max_count);
        targets.extend(local_members.into_iter().take(max_count));

        if targets.len() < max_count {
            let remaining = max_count - targets.len();
            targets.extend(remote_members.into_iter().take(remaining));
        }

        targets
    }

    /// Select one member per datacenter for cross-DC replication
    pub fn select_cross_dc_replication_targets(
        &self,
        exclude_self: Option<&str>,
    ) -> Vec<Arc<Member>> {
        if !self.config.cross_dc_replication_enabled {
            return Vec::new();
        }

        let mut targets = Vec::new();

        for entry in self.members_by_dc.iter() {
            // Skip local datacenter
            if entry.key() == &self.config.local_datacenter {
                continue;
            }

            // Select the healthiest member with highest locality weight
            let members = entry.value();
            let best = members
                .iter()
                .filter(|m| m.is_healthy())
                .filter(|m| exclude_self.is_none_or(|s| m.address != s))
                .max_by(|a, b| {
                    a.locality_weight()
                        .partial_cmp(&b.locality_weight())
                        .unwrap_or(std::cmp::Ordering::Equal)
                });

            if let Some(member) = best {
                targets.push(member.clone());
            } else {
                warn!(
                    "No healthy member available in datacenter {} for cross-DC replication",
                    entry.key()
                );
            }
        }

        targets
    }

    /// Check if a member is in the local datacenter
    pub fn is_local_member(&self, member: &Member) -> bool {
        member.datacenter() == self.config.local_datacenter
    }

    /// Check if a member is in the local region
    pub fn is_local_region(&self, member: &Member) -> bool {
        member.region() == self.config.local_region
    }

    /// Check if a member is in the local zone
    pub fn is_local_zone(&self, member: &Member) -> bool {
        member.zone() == self.config.local_zone
    }

    /// Get datacenter statistics
    pub fn get_statistics(&self) -> DatacenterStatistics {
        let local_dc_info = self.datacenter_info.get(&self.config.local_datacenter);

        DatacenterStatistics {
            total_datacenters: self.datacenter_info.len(),
            total_regions: self.get_all_regions().len(),
            total_members: self.members_by_dc.iter().map(|e| e.value().len()).sum(),
            local_datacenter: self.config.local_datacenter.clone(),
            local_members: local_dc_info
                .as_ref()
                .map(|i| i.total_member_count)
                .unwrap_or(0),
            local_healthy_members: local_dc_info
                .as_ref()
                .map(|i| i.healthy_member_count)
                .unwrap_or(0),
            remote_datacenters: self.datacenter_info.len().saturating_sub(1),
        }
    }

    /// Get all unique regions
    fn get_all_regions(&self) -> HashSet<String> {
        self.datacenter_info
            .iter()
            .map(|e| e.value().region.clone())
            .collect()
    }
}

/// Statistics about datacenter topology
#[derive(Debug, Clone)]
pub struct DatacenterStatistics {
    /// Total number of datacenters
    pub total_datacenters: usize,
    /// Total number of regions
    pub total_regions: usize,
    /// Total number of members across all datacenters
    pub total_members: usize,
    /// Local datacenter name
    pub local_datacenter: String,
    /// Number of members in local datacenter
    pub local_members: usize,
    /// Number of healthy members in local datacenter
    pub local_healthy_members: usize,
    /// Number of remote datacenters
    pub remote_datacenters: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use batata_api::model::MemberBuilder;

    fn create_member(ip: &str, port: u16, dc: &str, region: &str, zone: &str) -> Arc<Member> {
        Arc::new(
            MemberBuilder::new(ip.to_string(), port)
                .datacenter(dc)
                .region(region)
                .zone(zone)
                .build(),
        )
    }

    #[test]
    fn test_datacenter_config_default() {
        let config = DatacenterConfig::default();
        assert_eq!(config.local_datacenter, "default");
        assert_eq!(config.local_region, "default");
        assert_eq!(config.local_zone, "default");
        assert!(config.cross_dc_replication_enabled);
    }

    #[test]
    fn test_datacenter_manager_local_members() {
        let config = DatacenterConfig {
            local_datacenter: "dc1".to_string(),
            local_region: "us-east".to_string(),
            local_zone: "zone-a".to_string(),
            ..Default::default()
        };

        let manager = DatacenterManager::new(config);

        let members = vec![
            create_member("192.168.1.1", 8848, "dc1", "us-east", "zone-a"),
            create_member("192.168.1.2", 8848, "dc1", "us-east", "zone-b"),
            create_member("192.168.2.1", 8848, "dc2", "us-west", "zone-a"),
        ];

        manager.update_members(members);

        let local = manager.get_local_members();
        assert_eq!(local.len(), 2);

        let remote = manager.get_remote_members();
        assert_eq!(remote.len(), 1);

        let stats = manager.get_statistics();
        assert_eq!(stats.total_datacenters, 2);
        assert_eq!(stats.local_members, 2);
        assert_eq!(stats.remote_datacenters, 1);
    }

    #[test]
    fn test_cross_dc_replication_targets() {
        let config = DatacenterConfig {
            local_datacenter: "dc1".to_string(),
            cross_dc_replication_enabled: true,
            ..Default::default()
        };

        let manager = DatacenterManager::new(config);

        let members = vec![
            create_member("192.168.1.1", 8848, "dc1", "us-east", "zone-a"),
            create_member("192.168.2.1", 8848, "dc2", "us-west", "zone-a"),
            create_member("192.168.3.1", 8848, "dc3", "eu-west", "zone-a"),
        ];

        manager.update_members(members);

        let targets = manager.select_cross_dc_replication_targets(None);
        assert_eq!(targets.len(), 2); // dc2 and dc3, not dc1 (local)
    }
}
