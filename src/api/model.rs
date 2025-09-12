use std::{
    collections::BTreeMap,
    fmt::{Display, Formatter},
    str::FromStr,
    sync::{Arc, RwLock},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub total_count: u64,
    pub page_number: u64,
    pub pages_available: u64,
    pub page_items: Vec<T>,
}

impl<T> Default for Page<T> {
    fn default() -> Self {
        Self {
            total_count: 0,
            page_number: 1,
            pages_available: 0,
            page_items: vec![],
        }
    }
}

impl<T> Page<T> {
    pub fn new(total_count: u64, page_number: u64, page_size: u64, page_items: Vec<T>) -> Self {
        Self {
            total_count: total_count,
            page_number: page_number,
            pages_available: (total_count as f64 / page_size as f64).ceil() as u64,
            page_items: page_items,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum NodeState {
    Starting,
    Up,
    Suspicious,
    Down,
    Isolation,
}

impl NodeState {
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeState::Starting => "STARTING",
            NodeState::Up => "UP",
            NodeState::Suspicious => "SUSPICIOUS",
            NodeState::Down => "DOWN",
            NodeState::Isolation => "ISOLATION",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "STARTING" => Ok(NodeState::Starting),
            "UP" => Ok(NodeState::Up),
            "SUSPICIOUS" => Ok(NodeState::Suspicious),
            "DOWN" => Ok(NodeState::Down),
            "htISOLATIONml" => Ok(NodeState::Isolation),
            _ => Err(format!("Invalid node state: {}", s)),
        }
    }
}

impl Default for NodeState {
    fn default() -> Self {
        NodeState::Up
    }
}

impl Display for NodeState {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for NodeState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        NodeState::from_str(s)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Member {
    pub ip: String,
    pub port: u16,
    pub state: NodeState,
    pub extend_info: Arc<RwLock<BTreeMap<String, serde_json::Value>>>,
    pub address: String,
    pub fail_access_cnt: i32,
}

impl Member {
    pub const RAFT_PORT: &str = "raftPort";
    pub const SITE_KEY: &str = "site";
    pub const AD_WEIGHT: &str = "adWeight";
    pub const WEIGHT: &str = "weight";
    pub const LAST_REFRESH_TIME: &str = "lastRefreshTime";
    pub const VERSION: &str = "version";
    pub const SUPPORT_REMOTE_C_TYPE: &str = "remoteConnectType";
    pub const READY_TO_UPGRADE: &str = "readyToUpgrade";
    pub const SUPPORT_GRAY_MODEL: &str = "supportGrayModel";
}

pub struct MemberBuilder {
    pub ip: String,
    pub port: u16,
    pub node_state: NodeState,
    pub extend_info: Arc<RwLock<BTreeMap<String, serde_json::Value>>>,
}

impl MemberBuilder {
    pub fn new(ip: String, port: u16) -> Self {
        let mut map: BTreeMap<String, Value> = BTreeMap::<String, serde_json::Value>::new();

        map.insert(
            String::from("site"),
            serde_json::Value::String(
                std::env::var("nacos.core.member.meta.site").unwrap_or(String::from("unknow")),
            ),
        );
        map.insert(
            String::from("adWeight"),
            serde_json::Value::String(
                std::env::var("nacos.core.member.meta.adWeight").unwrap_or(String::from("0")),
            ),
        );
        map.insert(
            String::from("weight"),
            serde_json::Value::String(
                std::env::var("nacos.core.member.meta.weight").unwrap_or(String::from("1")),
            ),
        );

        MemberBuilder {
            ip: ip,
            port: port,
            node_state: NodeState::default(),
            extend_info: Arc::<RwLock<BTreeMap<String, serde_json::Value>>>::new(RwLock::new(map)),
        }
    }

    pub fn ip(mut self, ip: String) -> Self {
        self.ip = ip;

        self
    }

    pub fn port(mut self, port: u16) -> Self {
        self.port = port;

        self
    }

    pub fn node_state(mut self, node_state: NodeState) -> Self {
        self.node_state = node_state;

        self
    }

    pub fn build(self) -> Member {
        Member {
            ip: self.ip.clone(),
            port: self.port,
            state: self.node_state,
            extend_info: self.extend_info,
            address: format!("{}:{}", self.ip.clone(), self.port),
            fail_access_cnt: 0,
        }
    }
}
