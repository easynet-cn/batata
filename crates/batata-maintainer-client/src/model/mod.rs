// Model types for maintainer client API responses

pub mod ai;
pub mod client;
pub mod cluster;
pub mod common;
pub mod config;
pub mod core;
pub mod import_export;
pub mod namespace;
pub mod naming;
pub mod service;

pub use ai::{
    AgentCard, AgentCardBasicInfo, AgentCardDetailInfo, AgentCardVersionInfo, AgentInterface,
    AgentVersionDetail, McpEndpointSpec, McpServerBasicInfo, McpServerDetailInfo,
    McpToolSpecification, ServerVersionDetail,
};
pub use client::{ClientPublisherInfo, ClientServiceInfo, ClientSubscriberInfo, ClientSummaryInfo};
pub use cluster::{
    ClusterHealthResponse, ClusterHealthSummary, Member, NamingAbility, NodeAbilities,
    SelfMemberResponse,
};
pub use common::Page;
pub use config::{
    ConfigBasicInfo, ConfigCloneInfo, ConfigDetailInfo, ConfigGrayInfo, ConfigHistoryBasicInfo,
    ConfigHistoryDetailInfo, ConfigListenerInfo,
};
pub use core::{ConnectionInfo, IdGeneratorInfo, IdInfo, ServerLoaderInfo, ServerLoaderMetrics};
pub use import_export::{ImportFailItem, ImportResult, SameConfigPolicy};
pub use namespace::Namespace;
pub use naming::Instance;
pub use service::{
    ClusterInfo, InstanceMetadataBatchResult, MetricsInfo, ServiceDetailInfo, ServiceView,
    SubscriberInfo,
};
