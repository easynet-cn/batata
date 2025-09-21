use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use derive_builder::Builder;
use tokio::sync::mpsc::Sender;
use tonic::Status;

use crate::grpc::Payload;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageParam {
    #[serde(default = "PageParam::default_page_no")]
    pub page_no: u64,
    #[serde(default = "PageParam::default_page_size")]
    pub page_size: u64,
}

impl PageParam {
    pub fn start(&self) -> u64 {
        (self.page_no - 1) * self.page_size
    }

    fn default_page_no() -> u64 {
        1
    }

    fn default_page_size() -> u64 {
        100
    }
}

/// Connection meta infomation
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionMeta {
    /// ConnectionType.
    #[builder(default)]
    pub connect_type: String,

    /// Client IP Address.
    #[builder(default)]
    pub client_ip: String,

    /// Remote IP Address.
    #[builder(default)]
    pub remote_ip: String,

    /// Remote IP Port.
    #[builder(default)]
    pub remote_port: u16,

    /// Local Ip Port.
    #[builder(default)]
    pub local_port: u16,

    /// Client version.
    #[builder(default)]
    pub version: String,

    /// Identify Unique connectionId.
    #[builder(default)]
    pub connection_id: String,

    /// Create time.
    #[builder(default)]
    pub create_time: i64,

    /// Last active time.
    #[builder(default)]
    pub last_active_time: i64,

    /// App name.
    #[builder(default)]
    pub app_name: String,

    /// Namespace id.
    #[builder(default)]
    pub namespace_id: String,

    /// Labels.
    #[builder(default)]
    pub labels: HashMap<String, String>,

    /// Tls protected.
    #[builder(default)]
    pub tls_protected: bool,
}

impl ConnectionMeta {
    pub fn builder() -> ConnectionMetaBuilder {
        ConnectionMetaBuilder::default()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    #[builder(default)]
    pub traced: bool,

    #[builder(default)]
    pub ability_table: HashMap<String, bool>,

    #[builder(default)]
    #[serde(flatten)]
    pub meta_info: ConnectionMeta,
}

impl Connection {
    pub fn builder() -> ConnectionBuilder {
        ConnectionBuilder::default()
    }
}

pub struct GrpcClient {
    pub connection: Connection,
    pub tx: Sender<Result<Payload, Status>>,
}

impl GrpcClient {
    pub fn new(connection: Connection, tx: Sender<Result<Payload, Status>>) -> Self {
        Self { connection, tx }
    }
}
