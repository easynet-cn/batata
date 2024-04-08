use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Namespace {
    pub namespace: String,
    pub namespace_show_name: String,
    pub namespace_desc: String,
    pub quota: i32,
    pub config_count: i32,
    pub type_: i32,
}

impl Default for Namespace {
    fn default() -> Self {
        Namespace {
            namespace: "".to_string(),
            namespace_show_name: "Public".to_string(),
            namespace_desc: "Public Namespace".to_string(),
            quota: 200,
            config_count: 0,
            type_: 0,
        }
    }
}
