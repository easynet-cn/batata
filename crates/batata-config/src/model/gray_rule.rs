//! Gray release rule model and matching logic
//!
//! Supports multiple gray rule types:
//! - Beta (IP-based): Match clients by IP address
//! - Tag (Label-based): Match clients by label/tag
//! - Percentage (Traffic-based): Match a percentage of traffic
//! - IpRange (Range-based): Match clients by IP CIDR ranges

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::net::IpAddr;

use serde::{Deserialize, Serialize};

/// Gray rule type constants
pub mod rule_type {
    pub const BETA: &str = "beta";
    pub const TAG: &str = "tag";
    pub const PERCENTAGE: &str = "percentage";
    pub const IP_RANGE: &str = "ip_range";
}

/// Gray rule version
pub const GRAY_RULE_VERSION: &str = "1.0.0";

/// Label keys for client matching
pub mod labels {
    pub const CLIENT_IP: &str = "ClientIp";
    pub const VIP_SERVER_TAG: &str = "vipServerTag";
    pub const APP_NAME: &str = "appName";
    pub const CLUSTER_NAME: &str = "clusterName";
}

/// Persisted gray rule information (JSON serialized)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GrayRulePersistInfo {
    /// Rule type: "beta", "tag", "percentage", "ip_range"
    #[serde(rename = "type")]
    pub rule_type: String,
    /// Rule version
    pub version: String,
    /// Rule expression (format depends on type)
    pub expr: String,
    /// Priority for matching (higher = checked first)
    pub priority: i32,
}

impl GrayRulePersistInfo {
    /// Create a new beta (IP-based) rule
    pub fn new_beta(ips: &str, priority: i32) -> Self {
        Self {
            rule_type: rule_type::BETA.to_string(),
            version: GRAY_RULE_VERSION.to_string(),
            expr: ips.to_string(),
            priority,
        }
    }

    /// Create a new tag-based rule
    pub fn new_tag(tag: &str, priority: i32) -> Self {
        Self {
            rule_type: rule_type::TAG.to_string(),
            version: GRAY_RULE_VERSION.to_string(),
            expr: tag.to_string(),
            priority,
        }
    }

    /// Create a new percentage-based rule
    pub fn new_percentage(percentage: u8, priority: i32) -> Self {
        Self {
            rule_type: rule_type::PERCENTAGE.to_string(),
            version: GRAY_RULE_VERSION.to_string(),
            expr: percentage.to_string(),
            priority,
        }
    }

    /// Create a new IP range rule (CIDR notation)
    pub fn new_ip_range(cidr_ranges: &str, priority: i32) -> Self {
        Self {
            rule_type: rule_type::IP_RANGE.to_string(),
            version: GRAY_RULE_VERSION.to_string(),
            expr: cidr_ranges.to_string(),
            priority,
        }
    }

    /// Parse from JSON string
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Serialize to JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

/// Gray rule trait for matching
pub trait GrayRule: Send + Sync {
    /// Check if the rule matches the given labels
    fn matches(&self, labels: &HashMap<String, String>) -> bool;

    /// Validate the rule expression
    fn is_valid(&self) -> bool;

    /// Get rule type
    fn rule_type(&self) -> &str;

    /// Get rule priority
    fn priority(&self) -> i32;
}

/// Beta (IP-based) gray rule
#[derive(Clone, Debug)]
pub struct BetaGrayRule {
    ips: HashSet<String>,
    priority: i32,
    valid: bool,
}

impl BetaGrayRule {
    pub const PRIORITY: i32 = i32::MAX;

    pub fn new(expr: &str, priority: i32) -> Self {
        let ips: HashSet<String> = expr
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let valid = !ips.is_empty();
        Self {
            ips,
            priority,
            valid,
        }
    }
}

impl GrayRule for BetaGrayRule {
    fn matches(&self, labels: &HashMap<String, String>) -> bool {
        if let Some(client_ip) = labels.get(labels::CLIENT_IP) {
            return self.ips.contains(client_ip);
        }
        false
    }

    fn is_valid(&self) -> bool {
        self.valid
    }

    fn rule_type(&self) -> &str {
        rule_type::BETA
    }

    fn priority(&self) -> i32 {
        self.priority
    }
}

/// Tag (Label-based) gray rule
#[derive(Clone, Debug)]
pub struct TagGrayRule {
    tag: String,
    priority: i32,
    valid: bool,
}

impl TagGrayRule {
    pub const PRIORITY: i32 = i32::MAX - 1;

    pub fn new(expr: &str, priority: i32) -> Self {
        let tag = expr.trim().to_string();
        let valid = !tag.is_empty();
        Self {
            tag,
            priority,
            valid,
        }
    }
}

impl GrayRule for TagGrayRule {
    fn matches(&self, labels: &HashMap<String, String>) -> bool {
        if let Some(server_tag) = labels.get(labels::VIP_SERVER_TAG) {
            return self.tag == *server_tag;
        }
        false
    }

    fn is_valid(&self) -> bool {
        self.valid
    }

    fn rule_type(&self) -> &str {
        rule_type::TAG
    }

    fn priority(&self) -> i32 {
        self.priority
    }
}

/// Percentage-based gray rule
#[derive(Clone, Debug)]
pub struct PercentageGrayRule {
    percentage: u8,
    priority: i32,
    valid: bool,
}

impl PercentageGrayRule {
    pub const PRIORITY: i32 = i32::MAX - 2;

    pub fn new(expr: &str, priority: i32) -> Self {
        let percentage = expr.trim().parse::<u8>().unwrap_or(0);
        let valid = percentage > 0 && percentage <= 100;
        Self {
            percentage,
            priority,
            valid,
        }
    }

    /// Hash-based percentage matching using client identifier
    fn hash_matches(&self, identifier: &str) -> bool {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        identifier.hash(&mut hasher);
        let hash = hasher.finish();
        (hash % 100) < self.percentage as u64
    }
}

impl GrayRule for PercentageGrayRule {
    fn matches(&self, labels: &HashMap<String, String>) -> bool {
        // Use client IP or any unique identifier for consistent hashing
        if let Some(client_ip) = labels.get(labels::CLIENT_IP) {
            return self.hash_matches(client_ip);
        }
        // Fallback to random matching if no identifier
        false
    }

    fn is_valid(&self) -> bool {
        self.valid
    }

    fn rule_type(&self) -> &str {
        rule_type::PERCENTAGE
    }

    fn priority(&self) -> i32 {
        self.priority
    }
}

/// IP range (CIDR-based) gray rule
#[derive(Clone, Debug)]
pub struct IpRangeGrayRule {
    ranges: Vec<IpRange>,
    priority: i32,
    valid: bool,
}

#[derive(Clone, Debug)]
struct IpRange {
    network: u32,
    mask: u32,
}

impl IpRangeGrayRule {
    pub const PRIORITY: i32 = i32::MAX - 3;

    pub fn new(expr: &str, priority: i32) -> Self {
        let mut ranges = Vec::new();
        let mut valid = true;

        for cidr in expr.split(',') {
            let cidr = cidr.trim();
            if cidr.is_empty() {
                continue;
            }

            if let Some(range) = Self::parse_cidr(cidr) {
                ranges.push(range);
            } else {
                valid = false;
                break;
            }
        }

        if ranges.is_empty() {
            valid = false;
        }

        Self {
            ranges,
            priority,
            valid,
        }
    }

    fn parse_cidr(cidr: &str) -> Option<IpRange> {
        let parts: Vec<&str> = cidr.split('/').collect();
        if parts.is_empty() || parts.len() > 2 {
            return None;
        }

        let ip: IpAddr = parts[0].parse().ok()?;
        let prefix_len: u8 = if parts.len() == 2 {
            parts[1].parse().ok()?
        } else {
            32
        };

        if prefix_len > 32 {
            return None;
        }

        if let IpAddr::V4(ipv4) = ip {
            let network = u32::from(ipv4);
            let mask = if prefix_len == 0 {
                0
            } else {
                !0u32 << (32 - prefix_len)
            };
            Some(IpRange {
                network: network & mask,
                mask,
            })
        } else {
            // IPv6 not supported yet
            None
        }
    }

    fn ip_in_ranges(&self, ip_str: &str) -> bool {
        let ip: IpAddr = match ip_str.parse() {
            Ok(ip) => ip,
            Err(_) => return false,
        };

        if let IpAddr::V4(ipv4) = ip {
            let ip_num = u32::from(ipv4);
            for range in &self.ranges {
                if (ip_num & range.mask) == range.network {
                    return true;
                }
            }
        }
        false
    }
}

impl GrayRule for IpRangeGrayRule {
    fn matches(&self, labels: &HashMap<String, String>) -> bool {
        if let Some(client_ip) = labels.get(labels::CLIENT_IP) {
            return self.ip_in_ranges(client_ip);
        }
        false
    }

    fn is_valid(&self) -> bool {
        self.valid
    }

    fn rule_type(&self) -> &str {
        rule_type::IP_RANGE
    }

    fn priority(&self) -> i32 {
        self.priority
    }
}

/// Create a gray rule from persist info
pub fn create_gray_rule(info: &GrayRulePersistInfo) -> Option<Box<dyn GrayRule>> {
    match info.rule_type.as_str() {
        rule_type::BETA => {
            let rule = BetaGrayRule::new(&info.expr, info.priority);
            if rule.is_valid() {
                Some(Box::new(rule))
            } else {
                None
            }
        }
        rule_type::TAG => {
            let rule = TagGrayRule::new(&info.expr, info.priority);
            if rule.is_valid() {
                Some(Box::new(rule))
            } else {
                None
            }
        }
        rule_type::PERCENTAGE => {
            let rule = PercentageGrayRule::new(&info.expr, info.priority);
            if rule.is_valid() {
                Some(Box::new(rule))
            } else {
                None
            }
        }
        rule_type::IP_RANGE => {
            let rule = IpRangeGrayRule::new(&info.expr, info.priority);
            if rule.is_valid() {
                Some(Box::new(rule))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Parse and create a gray rule from JSON string
pub fn parse_gray_rule(json: &str) -> Option<Box<dyn GrayRule>> {
    let info = GrayRulePersistInfo::from_json(json).ok()?;
    create_gray_rule(&info)
}

/// Match labels against a gray rule JSON string
pub fn matches_gray_rule(gray_rule_json: &str, labels: &HashMap<String, String>) -> bool {
    if let Some(rule) = parse_gray_rule(gray_rule_json) {
        return rule.matches(labels);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_beta_gray_rule() {
        let rule = BetaGrayRule::new("192.168.1.1, 192.168.1.2", 100);
        assert!(rule.is_valid());

        let mut labels = HashMap::new();
        labels.insert(labels::CLIENT_IP.to_string(), "192.168.1.1".to_string());
        assert!(rule.matches(&labels));

        labels.insert(labels::CLIENT_IP.to_string(), "192.168.1.3".to_string());
        assert!(!rule.matches(&labels));
    }

    #[test]
    fn test_tag_gray_rule() {
        let rule = TagGrayRule::new("v2.0-testing", 100);
        assert!(rule.is_valid());

        let mut labels = HashMap::new();
        labels.insert(
            labels::VIP_SERVER_TAG.to_string(),
            "v2.0-testing".to_string(),
        );
        assert!(rule.matches(&labels));

        labels.insert(labels::VIP_SERVER_TAG.to_string(), "v1.0".to_string());
        assert!(!rule.matches(&labels));
    }

    #[test]
    fn test_percentage_gray_rule() {
        let rule = PercentageGrayRule::new("50", 100);
        assert!(rule.is_valid());

        // Test deterministic hashing
        let mut labels = HashMap::new();
        labels.insert(labels::CLIENT_IP.to_string(), "192.168.1.1".to_string());
        let result1 = rule.matches(&labels);

        // Same IP should always get same result
        let result2 = rule.matches(&labels);
        assert_eq!(result1, result2);

        // Invalid percentage
        let invalid_rule = PercentageGrayRule::new("101", 100);
        assert!(!invalid_rule.is_valid());
    }

    #[test]
    fn test_ip_range_gray_rule() {
        let rule = IpRangeGrayRule::new("192.168.1.0/24, 10.0.0.0/8", 100);
        assert!(rule.is_valid());

        let mut labels = HashMap::new();
        labels.insert(labels::CLIENT_IP.to_string(), "192.168.1.100".to_string());
        assert!(rule.matches(&labels));

        labels.insert(labels::CLIENT_IP.to_string(), "10.1.2.3".to_string());
        assert!(rule.matches(&labels));

        labels.insert(labels::CLIENT_IP.to_string(), "172.16.0.1".to_string());
        assert!(!rule.matches(&labels));
    }

    #[test]
    fn test_gray_rule_persist_info() {
        let info = GrayRulePersistInfo::new_beta("192.168.1.1,192.168.1.2", 100);
        let json = info.to_json().unwrap();
        assert!(json.contains("beta"));
        assert!(json.contains("192.168.1.1"));

        let parsed = GrayRulePersistInfo::from_json(&json).unwrap();
        assert_eq!(parsed.rule_type, rule_type::BETA);
        assert_eq!(parsed.priority, 100);
    }

    #[test]
    fn test_parse_gray_rule() {
        let json = r#"{"type":"beta","version":"1.0.0","expr":"192.168.1.1","priority":100}"#;
        let rule = parse_gray_rule(json);
        assert!(rule.is_some());

        let mut labels = HashMap::new();
        labels.insert(labels::CLIENT_IP.to_string(), "192.168.1.1".to_string());
        assert!(rule.unwrap().matches(&labels));
    }

    #[test]
    fn test_matches_gray_rule() {
        let json = r#"{"type":"tag","version":"1.0.0","expr":"canary","priority":100}"#;
        let mut labels = HashMap::new();
        labels.insert(labels::VIP_SERVER_TAG.to_string(), "canary".to_string());
        assert!(matches_gray_rule(json, &labels));
    }
}
