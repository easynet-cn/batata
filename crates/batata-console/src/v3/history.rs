//! Configuration history console endpoints

use std::sync::Arc;

use actix_web::{Responder, Scope, get, web};
use serde::{Deserialize, Serialize};

use batata_api::Page;
use batata_config::ConfigHistoryInfo;

use crate::datasource::ConsoleDataSource;

use super::config::DEFAULT_NAMESPACE_ID;
use super::namespace::ApiResult;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct FindOneParam {
    pub data_id: String,
    pub group_name: String,
    pub namespace_id: String,
    pub nid: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchParam {
    pub data_id: String,
    pub group_name: String,
    pub tenant: Option<String>,
    pub namespace_id: Option<String>,
    pub page_no: u64,
    pub page_size: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FindConfigsByNamespaceIdParam {
    pub namespace_id: String,
}

/// Config history basic info for API response
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigHistoryBasicInfo {
    pub id: u64,
    pub data_id: String,
    pub group: String,
    pub tenant: String,
    pub op_type: String,
    pub publish_type: String,
    pub gray_name: String,
    pub src_user: String,
    pub src_ip: String,
    pub created_time: i64,
    pub last_modified_time: i64,
}

impl From<ConfigHistoryInfo> for ConfigHistoryBasicInfo {
    fn from(info: ConfigHistoryInfo) -> Self {
        Self {
            id: info.id,
            data_id: info.data_id,
            group: info.group,
            tenant: info.tenant,
            op_type: info.op_type,
            publish_type: info.publish_type,
            gray_name: info.gray_name,
            src_user: info.src_user,
            src_ip: info.src_ip,
            created_time: info.created_time,
            last_modified_time: info.last_modified_time,
        }
    }
}

/// Config history detail info for API response
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigHistoryDetailInfo {
    pub id: u64,
    pub data_id: String,
    pub group: String,
    pub tenant: String,
    pub content: String,
    pub md5: String,
    pub app_name: String,
    pub op_type: String,
    pub publish_type: String,
    pub gray_name: String,
    pub ext_info: String,
    pub src_user: String,
    pub src_ip: String,
    pub created_time: i64,
    pub last_modified_time: i64,
    pub encrypted_data_key: String,
}

impl From<ConfigHistoryInfo> for ConfigHistoryDetailInfo {
    fn from(info: ConfigHistoryInfo) -> Self {
        Self {
            id: info.id,
            data_id: info.data_id,
            group: info.group,
            tenant: info.tenant,
            content: info.content,
            md5: info.md5,
            app_name: info.app_name,
            op_type: info.op_type,
            publish_type: info.publish_type,
            gray_name: info.gray_name,
            ext_info: info.ext_info,
            src_user: info.src_user,
            src_ip: info.src_ip,
            created_time: info.created_time,
            last_modified_time: info.last_modified_time,
            encrypted_data_key: info.encrypted_data_key,
        }
    }
}

/// Config basic info for configs by namespace response
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigBasicInfo {
    pub id: Option<u64>,
    pub namespace_id: String,
    pub group_name: String,
    pub data_id: String,
    pub md5: Option<String>,
    pub r#type: String,
    pub app_name: String,
    pub create_time: i64,
    pub modify_time: i64,
}

impl From<batata_config::ConfigInfoWrapper> for ConfigBasicInfo {
    fn from(wrapper: batata_config::ConfigInfoWrapper) -> Self {
        Self {
            id: wrapper.id,
            namespace_id: wrapper.namespace_id,
            group_name: wrapper.group_name,
            data_id: wrapper.data_id,
            md5: wrapper.md5,
            r#type: wrapper.r#type,
            app_name: wrapper.app_name,
            create_time: wrapper.create_time,
            modify_time: wrapper.modify_time,
        }
    }
}

#[get("")]
pub async fn find_one(
    datasource: web::Data<Arc<dyn ConsoleDataSource>>,
    params: web::Query<FindOneParam>,
) -> impl Responder {
    match datasource
        .history_get(
            params.nid,
            &params.data_id,
            &params.group_name,
            &params.namespace_id,
        )
        .await
    {
        Ok(result) => {
            let config_info = result.map(ConfigHistoryDetailInfo::from);
            ApiResult::<Option<ConfigHistoryDetailInfo>>::http_success(config_info)
        }
        Err(e) => {
            tracing::error!("Failed to find history by id: {}", e);
            ApiResult::<String>::http_response(
                500,
                500,
                format!("Failed to find history: {}", e),
                String::new(),
            )
        }
    }
}

#[get("list")]
pub async fn search(
    datasource: web::Data<Arc<dyn ConsoleDataSource>>,
    params: web::Query<SearchParam>,
) -> impl Responder {
    let data_id = &params.data_id;
    let group_name = &params.group_name;
    let mut namespace_id = params.tenant.clone().unwrap_or_default();

    if namespace_id.is_empty() {
        namespace_id = params
            .namespace_id
            .clone()
            .unwrap_or_else(|| DEFAULT_NAMESPACE_ID.to_string());
    }

    match datasource
        .history_list(
            data_id,
            group_name,
            &namespace_id,
            params.page_no,
            params.page_size,
        )
        .await
    {
        Ok(result) => {
            let page_result = Page::<ConfigHistoryBasicInfo>::new(
                result.total_count,
                result.page_number,
                result.pages_available,
                result
                    .page_items
                    .into_iter()
                    .map(ConfigHistoryBasicInfo::from)
                    .collect(),
            );
            ApiResult::<Page<ConfigHistoryBasicInfo>>::http_success(page_result)
        }
        Err(e) => {
            tracing::error!("Failed to search history: {}", e);
            ApiResult::<String>::http_response(
                500,
                500,
                format!("Failed to search history: {}", e),
                String::new(),
            )
        }
    }
}

#[get("configs")]
pub async fn find_configs_by_namespace_id(
    datasource: web::Data<Arc<dyn ConsoleDataSource>>,
    params: web::Query<FindConfigsByNamespaceIdParam>,
) -> impl Responder {
    match datasource
        .history_configs_by_namespace(&params.namespace_id)
        .await
    {
        Ok(result) => {
            let config_infos = result
                .into_iter()
                .map(ConfigBasicInfo::from)
                .collect::<Vec<ConfigBasicInfo>>();
            ApiResult::<Vec<ConfigBasicInfo>>::http_success(config_infos)
        }
        Err(e) => {
            tracing::error!("Failed to find configs by namespace: {}", e);
            ApiResult::<String>::http_response(
                500,
                500,
                format!("Failed to find configs: {}", e),
                String::new(),
            )
        }
    }
}

pub fn routes() -> Scope {
    web::scope("/cs/history")
        .service(find_one)
        .service(search)
        .service(find_configs_by_namespace_id)
}
