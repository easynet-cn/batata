use std::sync::Arc;
use std::net::IpAddr;
use std::collections::HashMap;

use crate::api::dto::GrayReleaseRuleDTO;
use crate::persistence::shared::StoredGrayReleaseRule;
use crate::persistence::traits::{ApolloPersistenceService, GrayReleasePersistence};
use chrono::Utc;
use serde_json::Value;

pub struct GrayReleaseRuleService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl GrayReleaseRuleService {
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    pub async fn create(&self, dto: GrayReleaseRuleDTO) -> Result<GrayReleaseRuleDTO, anyhow::Error> {
        let existing = self.persistence.get_by_namespace(&dto.app_id, &dto.cluster_name, &dto.namespace_name).await?;

        if existing.is_some() {
            return Err(anyhow::anyhow!("Gray release rule already exists: {}/{}/{}/{}", dto.app_id, dto.cluster_name, dto.namespace_name, dto.branch_name));
        }

        let now = Utc::now().timestamp_millis();
        let created_by = dto.data_change_created_by.clone().unwrap_or_default();

        let stored = StoredGrayReleaseRule {
            id: 0,
            app_id: dto.app_id.clone(),
            cluster_name: dto.cluster_name.clone(),
            namespace_name: dto.namespace_name.clone(),
            branch_name: dto.branch_name.clone(),
            rules: dto.rules.clone().unwrap_or_else(|| "[]".to_string()),
            release_id: dto.release_id,
            branch_status: Some(dto.branch_status.map(|v| v as i16).unwrap_or(1)),
            is_deleted: false,
            deleted_at: 0,
            data_change_created_by: created_by,
            data_change_created_time: now,
            data_change_last_modified_by: dto.data_change_last_modified_by.clone(),
            data_change_last_time: Some(now),
        };

        let created = self.persistence.create(stored).await?;
        Ok(created.into())
    }

    pub async fn get(&self, app_id: &str, cluster_name: &str, namespace_name: &str, branch_name: &str) -> Result<Option<GrayReleaseRuleDTO>, anyhow::Error> {
        let stored_list = self.persistence.list_by_app(app_id).await?;
        let found = stored_list.into_iter()
            .find(|s| s.cluster_name == cluster_name && s.namespace_name == namespace_name && s.branch_name == branch_name && !s.is_deleted);
        Ok(found.map(|s| s.into()))
    }

    pub async fn list_by_namespace(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> Result<Vec<GrayReleaseRuleDTO>, anyhow::Error> {
        let stored_list = self.persistence.list_by_app(app_id).await?;
        Ok(stored_list
            .into_iter()
            .filter(|s| s.cluster_name == cluster_name && s.namespace_name == namespace_name && !s.is_deleted)
            .map(|s| s.into())
            .collect())
    }

    pub async fn update(&self, app_id: &str, cluster_name: &str, namespace_name: &str, branch_name: &str, dto: GrayReleaseRuleDTO) -> Result<(), anyhow::Error> {
        let stored_list = self.persistence.list_by_app(app_id).await?;
        let existing = stored_list.into_iter()
            .find(|s| s.cluster_name == cluster_name && s.namespace_name == namespace_name && s.branch_name == branch_name && !s.is_deleted)
            .ok_or_else(|| anyhow::anyhow!("Gray release rule not found"))?;

        let now = Utc::now().timestamp_millis();

        let rules = dto.rules.unwrap_or(existing.rules);
        self.persistence.update_rules(existing.id, rules, dto.release_id).await?;
        Ok(())
    }

    pub async fn delete(&self, app_id: &str, cluster_name: &str, namespace_name: &str, branch_name: &str, _operator: &str) -> Result<(), anyhow::Error> {
        let stored_list = self.persistence.list_by_app(app_id).await?;
        let existing = stored_list.into_iter()
            .find(|s| s.cluster_name == cluster_name && s.namespace_name == namespace_name && s.branch_name == branch_name && !s.is_deleted)
            .ok_or_else(|| anyhow::anyhow!("Gray release rule not found"))?;

        self.persistence.delete(existing.id).await?;
        Ok(())
    }

    pub async fn match_gray_release_rule(&self, app_id: &str, cluster_name: &str, namespace_name: &str, client_ip: &str) -> Result<Option<i64>, anyhow::Error> {
        let rules = self.persistence.list_by_app(app_id).await?;
        let active_rules: Vec<_> = rules.into_iter()
            .filter(|r| r.cluster_name == cluster_name && r.namespace_name == namespace_name && !r.is_deleted && r.branch_status == Some(1))
            .collect();

        if active_rules.is_empty() {
            return Ok(None);
        }

        let client_ip_addr: IpAddr = client_ip.parse().unwrap_or(IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)));

        for rule in active_rules {
            let rules_json: Value = serde_json::from_str(&rule.rules).unwrap_or_default();
            if let Some(rules_array) = rules_json.as_array() {
                for rule_item in rules_array {
                    if let Some(ip_range) = rule_item.get("ip").and_then(|v| v.as_str()) {
                        if self.is_ip_in_range(&client_ip_addr, ip_range) {
                            return Ok(Some(rule.release_id));
                        }
                    }
                    if let Some(ip_segment) = rule_item.get("ipSegment").and_then(|v| v.as_str()) {
                        if self.is_ip_in_range(&client_ip_addr, ip_segment) {
                            return Ok(Some(rule.release_id));
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    pub async fn match_gray_release_rule_with_context(&self, app_id: &str, cluster_name: &str, namespace_name: &str, client_ip: &str, labels: Option<&HashMap<String, String>>, headers: Option<&HashMap<String, String>>) -> Result<Option<i64>, anyhow::Error> {
        let rules = self.persistence.list_by_app(app_id).await?;
        let active_rules: Vec<_> = rules.into_iter()
            .filter(|r| r.cluster_name == cluster_name && r.namespace_name == namespace_name && !r.is_deleted && r.branch_status == Some(1))
            .collect();

        if active_rules.is_empty() {
            return Ok(None);
        }

        let client_ip_addr: IpAddr = client_ip.parse().unwrap_or(IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)));

        for rule in active_rules {
            let rules_json: Value = serde_json::from_str(&rule.rules).unwrap_or_default();
            if let Some(rules_array) = rules_json.as_array() {
                if self.match_rules_array(&client_ip_addr, rules_array, labels, headers) {
                    return Ok(Some(rule.release_id));
                }
            }
        }

        Ok(None)
    }

    fn match_rules_array(&self, ip: &IpAddr, rules_array: &[Value], labels: Option<&HashMap<String, String>>, headers: Option<&HashMap<String, String>>) -> bool {
        for rule_item in rules_array {
            if let Some(rule_type) = rule_item.get("type").and_then(|v| v.as_str()) {
                match rule_type {
                    "IP" | "ip" => {
                        if let Some(ip_range) = rule_item.get("ip").and_then(|v| v.as_str()) {
                            if self.is_ip_in_range(ip, ip_range) {
                                return true;
                            }
                        }
                        if let Some(ip_segment) = rule_item.get("ipSegment").and_then(|v| v.as_str()) {
                            if self.is_ip_in_range(ip, ip_segment) {
                                return true;
                            }
                        }
                    }
                    "LABEL" | "label" => {
                        if let Some(label_key) = rule_item.get("label").and_then(|v| v.as_str()) {
                            if let Some(label_value) = rule_item.get("value").and_then(|v| v.as_str()) {
                                if let Some(labels) = labels {
                                    if let Some(val) = labels.get(label_key) {
                                        if val == label_value || label_value == "*" {
                                            return true;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    "HEADER" | "header" => {
                        if let Some(header_key) = rule_item.get("header").and_then(|v| v.as_str()) {
                            if let Some(header_value) = rule_item.get("value").and_then(|v| v.as_str()) {
                                if let Some(headers) = headers {
                                    if let Some(val) = headers.get(header_key) {
                                        if val == header_value || header_value == "*" {
                                            return true;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        false
    }

    fn is_ip_in_range(&self, ip: &IpAddr, ip_range: &str) -> bool {
        if let IpAddr::V4(ip_v4) = ip {
            if ip_range.contains('/') {
                let parts: Vec<&str> = ip_range.split('/').collect();
                if parts.len() == 2 {
                    if let (Ok(network), Ok(prefix)) = (parts[0].parse::<std::net::Ipv4Addr>(), parts[1].parse::<u8>()) {
                        let mask = u32::MAX << (32 - prefix);
                        let ip_u32 = u32::from(*ip_v4);
                        let network_u32 = u32::from(network);
                        return (ip_u32 & mask) == (network_u32 & mask);
                    }
                }
            } else if ip_range == ip.to_string() {
                return true;
            } else if ip_range.contains('-') {
                let parts: Vec<&str> = ip_range.split('-').collect();
                if parts.len() == 2 {
                    if let (Ok(start), Ok(end)) = (parts[0].parse::<std::net::Ipv4Addr>(), parts[1].parse::<std::net::Ipv4Addr>()) {
                        let ip_u32 = u32::from(*ip_v4);
                        let start_u32 = u32::from(start);
                        let end_u32 = u32::from(end);
                        return ip_u32 >= start_u32 && ip_u32 <= end_u32;
                    }
                }
            }
        }
        false
    }
}

impl From<StoredGrayReleaseRule> for GrayReleaseRuleDTO {
    fn from(stored: StoredGrayReleaseRule) -> Self {
        Self {
            id: Some(stored.id),
            app_id: stored.app_id,
            cluster_name: stored.cluster_name,
            namespace_name: stored.namespace_name,
            branch_name: stored.branch_name,
            rules: Some(stored.rules),
            release_id: stored.release_id,
            branch_status: stored.branch_status.map(|bs| bs as i16),
            priority: None,
            data_change_created_by: Some(stored.data_change_created_by),
            data_change_last_modified_by: stored.data_change_last_modified_by,
            data_change_created_time: Some(format_timestamp(stored.data_change_created_time)),
        }
    }
}

fn format_timestamp(ts: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ts)
        .unwrap_or_default()
        .format("%Y-%m-%dT%H:%M:%S%.f+00:00")
        .to_string()
}