// Configuration management API models
// This file re-exports types from batata-server-common and batata-api

// Re-export all config API types from batata-api
pub use batata_api::config::{
    ClientConfigMetricRequest, ClientConfigMetricResponse, ConfigBatchListenRequest,
    ConfigChangeBatchListenResponse, ConfigChangeClusterSyncRequest,
    ConfigChangeClusterSyncResponse, ConfigChangeNotifyRequest, ConfigChangeNotifyResponse,
    ConfigCloneInfo, ConfigContext, ConfigFuzzyWatchChangeNotifyRequest,
    ConfigFuzzyWatchChangeNotifyResponse, ConfigFuzzyWatchRequest, ConfigFuzzyWatchResponse,
    ConfigFuzzyWatchSyncRequest, ConfigFuzzyWatchSyncResponse, ConfigListenContext,
    ConfigListenerInfo, ConfigPublishRequest, ConfigPublishResponse, ConfigQueryRequest,
    ConfigQueryResponse, ConfigRemoveRequest, ConfigRemoveResponse, ConfigRequest, Context,
    FuzzyWatchNotifyRequest, MetricsKey, SameConfigPolicy,
};

// Re-export nested console API config types from server-common
pub use batata_server_common::console::api_model::{
    ConfigBasicInfo, ConfigDetailInfo, ConfigGrayInfo, ConfigHistoryBasicInfo,
    ConfigHistoryDetailInfo,
};
