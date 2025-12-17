// Config module gRPC handlers
// Implements handlers for configuration management requests

use std::sync::Arc;

use tonic::Status;

use crate::{
    api::{
        config::model::{
            ClientConfigMetricRequest, ClientConfigMetricResponse, ConfigBatchListenRequest,
            ConfigChangeBatchListenResponse, ConfigChangeClusterSyncRequest,
            ConfigChangeClusterSyncResponse, ConfigChangeNotifyRequest, ConfigChangeNotifyResponse,
            ConfigContext, ConfigFuzzyWatchChangeNotifyRequest,
            ConfigFuzzyWatchChangeNotifyResponse, ConfigFuzzyWatchRequest, ConfigFuzzyWatchResponse,
            ConfigFuzzyWatchSyncRequest, ConfigFuzzyWatchSyncResponse, ConfigPublishRequest,
            ConfigPublishResponse, ConfigQueryRequest, ConfigQueryResponse, ConfigRemoveRequest,
            ConfigRemoveResponse,
        },
        grpc::{Metadata, Payload},
        remote::model::{RequestTrait, Response, ResponseCode, ResponseTrait},
    },
    core::model::Connection,
    model::common::AppState,
    service::{config, rpc::PayloadHandler},
};

// Handler for ConfigQueryRequest - queries configuration by dataId, group, tenant
#[derive(Clone)]
pub struct ConfigQueryHandler {
    pub app_state: Arc<AppState>,
}

#[tonic::async_trait]
impl PayloadHandler for ConfigQueryHandler {
    async fn handle(&self, connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = ConfigQueryRequest::from(payload);
        let request_id = request.request_id();

        let data_id = &request.config_request.data_id;
        let group = &request.config_request.group;
        let tenant = &request.config_request.tenant;

        let db = &self.app_state.database_connection;

        match config::find_one(db, data_id, group, tenant).await {
            Ok(Some(config_info)) => {
                let mut response = ConfigQueryResponse::new();
                response.response.request_id = request_id;
                response.content = config_info.config_info.config_info_base.content;
                response.md5 = config_info.config_info.config_info_base.md5;
                response.content_type = config_info.config_info.r#type;
                response.encrypted_data_key =
                    config_info.config_info.config_info_base.encrypted_data_key;
                response.last_modified = config_info.modify_time;

                let metadata = Metadata {
                    r#type: response.response_type().to_string(),
                    ..Default::default()
                };

                Ok(response.into_payload(Some(metadata)))
            }
            Ok(None) => {
                let mut response = ConfigQueryResponse::new();
                response.response.request_id = request_id;
                response.response.result_code = ConfigQueryResponse::CONFIG_NOT_FOUND;
                response.response.error_code = ConfigQueryResponse::CONFIG_NOT_FOUND;
                response.response.success = false;
                response.response.message = "config not found".to_string();

                let metadata = Metadata {
                    r#type: response.response_type().to_string(),
                    ..Default::default()
                };

                Ok(response.into_payload(Some(metadata)))
            }
            Err(e) => {
                let mut response = ConfigQueryResponse::new();
                response.response.request_id = request_id;
                response.response.result_code = ResponseCode::Fail.code();
                response.response.error_code = ResponseCode::Fail.code();
                response.response.success = false;
                response.response.message = e.to_string();

                let metadata = Metadata {
                    r#type: response.response_type().to_string(),
                    ..Default::default()
                };

                Ok(response.into_payload(Some(metadata)))
            }
        }
    }

    fn can_handle(&self) -> &'static str {
        "ConfigQueryRequest"
    }
}

// Handler for ConfigPublishRequest - publishes/updates configuration
#[derive(Clone)]
pub struct ConfigPublishHandler {
    pub app_state: Arc<AppState>,
}

#[tonic::async_trait]
impl PayloadHandler for ConfigPublishHandler {
    async fn handle(&self, connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = ConfigPublishRequest::from(payload);
        let request_id = request.request_id();

        let data_id = &request.config_request.data_id;
        let group = &request.config_request.group;
        let tenant = &request.config_request.tenant;
        let content = &request.content;

        // Extract additional params from addition_map
        let app_name = request
            .addition_map
            .get("appName")
            .map(|s| s.as_str())
            .unwrap_or("");
        let config_tags = request
            .addition_map
            .get("configTags")
            .map(|s| s.as_str())
            .unwrap_or("");
        let desc = request
            .addition_map
            .get("desc")
            .map(|s| s.as_str())
            .unwrap_or("");
        let r#use = request
            .addition_map
            .get("use")
            .map(|s| s.as_str())
            .unwrap_or("");
        let effect = request
            .addition_map
            .get("effect")
            .map(|s| s.as_str())
            .unwrap_or("");
        let r#type = request
            .addition_map
            .get("type")
            .map(|s| s.as_str())
            .unwrap_or("");
        let schema = request
            .addition_map
            .get("schema")
            .map(|s| s.as_str())
            .unwrap_or("");
        let encrypted_data_key = request
            .addition_map
            .get("encryptedDataKey")
            .map(|s| s.as_str())
            .unwrap_or("");

        let src_user = connection.meta_info.app_name.as_str();
        let src_ip = connection.meta_info.client_ip.as_str();

        let db = &self.app_state.database_connection;

        match config::create_or_update(
            db,
            data_id,
            group,
            tenant,
            content,
            app_name,
            src_user,
            src_ip,
            config_tags,
            desc,
            r#use,
            effect,
            r#type,
            schema,
            encrypted_data_key,
        )
        .await
        {
            Ok(_) => {
                let mut response = ConfigPublishResponse::new();
                response.response.request_id = request_id;

                let metadata = Metadata {
                    r#type: response.response_type().to_string(),
                    ..Default::default()
                };

                Ok(response.into_payload(Some(metadata)))
            }
            Err(e) => {
                let mut response = ConfigPublishResponse::new();
                response.response.request_id = request_id;
                response.response.result_code = ResponseCode::Fail.code();
                response.response.error_code = ResponseCode::Fail.code();
                response.response.success = false;
                response.response.message = e.to_string();

                let metadata = Metadata {
                    r#type: response.response_type().to_string(),
                    ..Default::default()
                };

                Ok(response.into_payload(Some(metadata)))
            }
        }
    }

    fn can_handle(&self) -> &'static str {
        "ConfigPublishRequest"
    }
}

// Handler for ConfigRemoveRequest - removes configuration
#[derive(Clone)]
pub struct ConfigRemoveHandler {
    pub app_state: Arc<AppState>,
}

#[tonic::async_trait]
impl PayloadHandler for ConfigRemoveHandler {
    async fn handle(&self, connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = ConfigRemoveRequest::from(payload);
        let request_id = request.request_id();

        let data_id = &request.config_request.data_id;
        let group = &request.config_request.group;
        let tenant = &request.config_request.tenant;
        let tag = &request.tag;

        let src_user = connection.meta_info.app_name.as_str();
        let src_ip = connection.meta_info.client_ip.as_str();

        let db = &self.app_state.database_connection;

        match config::delete(db, data_id, group, tenant, "", src_ip, src_user, "rpc").await {
            Ok(_) => {
                let mut response = ConfigRemoveResponse::new();
                response.response.request_id = request_id;

                let metadata = Metadata {
                    r#type: response.response_type().to_string(),
                    ..Default::default()
                };

                Ok(response.into_payload(Some(metadata)))
            }
            Err(e) => {
                let mut response = ConfigRemoveResponse::new();
                response.response.request_id = request_id;
                response.response.result_code = ResponseCode::Fail.code();
                response.response.error_code = ResponseCode::Fail.code();
                response.response.success = false;
                response.response.message = e.to_string();

                let metadata = Metadata {
                    r#type: response.response_type().to_string(),
                    ..Default::default()
                };

                Ok(response.into_payload(Some(metadata)))
            }
        }
    }

    fn can_handle(&self) -> &'static str {
        "ConfigRemoveRequest"
    }
}

// Handler for ConfigBatchListenRequest - batch listen for config changes
#[derive(Clone)]
pub struct ConfigBatchListenHandler {
    pub app_state: Arc<AppState>,
}

#[tonic::async_trait]
impl PayloadHandler for ConfigBatchListenHandler {
    async fn handle(&self, connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = ConfigBatchListenRequest::from(payload);
        let request_id = request.request_id();

        let db = &self.app_state.database_connection;
        let mut changed_configs = Vec::new();

        // Check each config in the listen list for changes
        for ctx in &request.config_listen_contexts {
            let data_id = &ctx.data_id;
            let group = &ctx.group;
            let tenant = &ctx.tenant;
            let client_md5 = &ctx.md5;

            // Query current config and compare MD5
            if let Ok(Some(config_info)) = config::find_one(db, data_id, group, tenant).await {
                let server_md5 = &config_info.config_info.config_info_base.md5;

                // If MD5 differs, config has changed
                if client_md5 != server_md5 {
                    changed_configs.push(ConfigContext {
                        data_id: data_id.clone(),
                        group: group.clone(),
                        tenant: tenant.clone(),
                    });
                }
            } else if request.listen {
                // Config doesn't exist but client is listening - report as changed
                changed_configs.push(ConfigContext {
                    data_id: data_id.clone(),
                    group: group.clone(),
                    tenant: tenant.clone(),
                });
            }
        }

        let mut response = ConfigChangeBatchListenResponse::new();
        response.response.request_id = request_id;
        response.changed_configs = changed_configs;

        let metadata = Metadata {
            r#type: response.response_type().to_string(),
            ..Default::default()
        };

        Ok(response.into_payload(Some(metadata)))
    }

    fn can_handle(&self) -> &'static str {
        "ConfigBatchListenRequest"
    }
}

// Handler for ConfigChangeNotifyRequest - notifies clients of config changes
// Note: This is typically a server-push request, handler acknowledges receipt
#[derive(Clone)]
pub struct ConfigChangeNotifyHandler {
    pub app_state: Arc<AppState>,
}

#[tonic::async_trait]
impl PayloadHandler for ConfigChangeNotifyHandler {
    async fn handle(&self, connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = ConfigChangeNotifyRequest::from(payload);
        let request_id = request.request_id();

        // This handler is for receiving acknowledgment from client
        // In server-side context, just acknowledge the request
        let mut response = ConfigChangeNotifyResponse::new();
        response.response.request_id = request_id;

        let metadata = Metadata {
            r#type: response.response_type().to_string(),
            ..Default::default()
        };

        Ok(response.into_payload(Some(metadata)))
    }

    fn can_handle(&self) -> &'static str {
        "ConfigChangeNotifyRequest"
    }
}

// Handler for ConfigChangeClusterSyncRequest - syncs config changes across cluster nodes
#[derive(Clone)]
pub struct ConfigChangeClusterSyncHandler {
    pub app_state: Arc<AppState>,
}

#[tonic::async_trait]
impl PayloadHandler for ConfigChangeClusterSyncHandler {
    async fn handle(&self, connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = ConfigChangeClusterSyncRequest::from(payload);
        let request_id = request.request_id();

        let data_id = &request.config_request.data_id;
        let group = &request.config_request.group;
        let tenant = &request.config_request.tenant;
        let _last_modified = request.last_modified;
        let _gray_name = &request.gray_name;

        // In a cluster sync scenario, this would trigger local cache refresh
        // For now, just acknowledge the sync request
        let mut response = ConfigChangeClusterSyncResponse::new();
        response.response.request_id = request_id;

        let metadata = Metadata {
            r#type: response.response_type().to_string(),
            ..Default::default()
        };

        Ok(response.into_payload(Some(metadata)))
    }

    fn can_handle(&self) -> &'static str {
        "ConfigChangeClusterSyncRequest"
    }
}

// Handler for ConfigFuzzyWatchRequest - handles fuzzy pattern watch for configs
#[derive(Clone)]
pub struct ConfigFuzzyWatchHandler {
    pub app_state: Arc<AppState>,
}

#[tonic::async_trait]
impl PayloadHandler for ConfigFuzzyWatchHandler {
    async fn handle(&self, connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = ConfigFuzzyWatchRequest::from(payload);
        let request_id = request.request_id();

        // FuzzyWatch allows watching configs matching a pattern
        // This would register the watch pattern for the connection
        let mut response = ConfigFuzzyWatchResponse::new();
        response.response.request_id = request_id;

        let metadata = Metadata {
            r#type: response.response_type().to_string(),
            ..Default::default()
        };

        Ok(response.into_payload(Some(metadata)))
    }

    fn can_handle(&self) -> &'static str {
        "ConfigFuzzyWatchRequest"
    }
}

// Handler for ConfigFuzzyWatchChangeNotifyRequest - notifies fuzzy watch changes
#[derive(Clone)]
pub struct ConfigFuzzyWatchChangeNotifyHandler {
    pub app_state: Arc<AppState>,
}

#[tonic::async_trait]
impl PayloadHandler for ConfigFuzzyWatchChangeNotifyHandler {
    async fn handle(&self, connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = ConfigFuzzyWatchChangeNotifyRequest::from(payload);
        let request_id = request.request_id();

        let mut response = ConfigFuzzyWatchChangeNotifyResponse::new();
        response.response.request_id = request_id;

        let metadata = Metadata {
            r#type: response.response_type().to_string(),
            ..Default::default()
        };

        Ok(response.into_payload(Some(metadata)))
    }

    fn can_handle(&self) -> &'static str {
        "ConfigFuzzyWatchChangeNotifyRequest"
    }
}

// Handler for ConfigFuzzyWatchSyncRequest - syncs fuzzy watch state
#[derive(Clone)]
pub struct ConfigFuzzyWatchSyncHandler {
    pub app_state: Arc<AppState>,
}

#[tonic::async_trait]
impl PayloadHandler for ConfigFuzzyWatchSyncHandler {
    async fn handle(&self, connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = ConfigFuzzyWatchSyncRequest::from(payload);
        let request_id = request.request_id();

        let mut response = ConfigFuzzyWatchSyncResponse::new();
        response.response.request_id = request_id;

        let metadata = Metadata {
            r#type: response.response_type().to_string(),
            ..Default::default()
        };

        Ok(response.into_payload(Some(metadata)))
    }

    fn can_handle(&self) -> &'static str {
        "ConfigFuzzyWatchSyncRequest"
    }
}

// Handler for ClientConfigMetricRequest - collects client config metrics
#[derive(Clone)]
pub struct ClientConfigMetricHandler {
    pub app_state: Arc<AppState>,
}

#[tonic::async_trait]
impl PayloadHandler for ClientConfigMetricHandler {
    async fn handle(&self, connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = ClientConfigMetricRequest::from(payload);
        let request_id = request.request_id();

        let mut response = ClientConfigMetricResponse::new();
        response.response.request_id = request_id;
        // Metrics would be populated based on request.metrics_keys

        let metadata = Metadata {
            r#type: response.response_type().to_string(),
            ..Default::default()
        };

        Ok(response.into_payload(Some(metadata)))
    }

    fn can_handle(&self) -> &'static str {
        "ClientConfigMetricRequest"
    }
}
