use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum NodeState {
    Starting,
    #[default]
    Up,
    Suspicious,
    Down,
    Isolation,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Member {
    pub ip: String,
    pub port: i32,
    pub state: NodeState,
    pub extend_info: BTreeMap<String, serde_json::Value>,
    pub address: String,
    pub fail_access_cnt: i32,
}

impl Member {
    pub fn new() -> Self {
        let mut m = BTreeMap::<String, serde_json::Value>::new();

        m.insert(
            String::from("site"),
            serde_json::Value::String(
                std::env::var("nacos.core.member.meta.site").unwrap_or(String::from("unknow")),
            ),
        );

        Self {
            ip: String::from(""),
            port: -1,
            state: NodeState::Up,
            address: String::from(""),
            extend_info: m,
            fail_access_cnt: 0,
        }
    }
}
