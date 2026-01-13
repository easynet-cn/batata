//! Configuration management console endpoints

use std::sync::Arc;

use actix_multipart::Multipart;
use actix_web::{HttpResponse, Responder, Scope, delete, get, post, web};
use chrono::Utc;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tracing::warn;

use batata_api::Page;
use batata_config::{ConfigAllInfo, ConfigBasicInfo, ConfigInfoGrayWrapper, SameConfigPolicy};

use crate::datasource::ConsoleDataSource;

use super::namespace::ApiResult;

pub const DEFAULT_NAMESPACE_ID: &str = "public";

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigForm {
    pub data_id: String,
    pub group_name: String,
    #[serde(default)]
    pub namespace_id: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub app_name: String,
    #[serde(default)]
    pub config_tags: String,
    #[serde(default)]
    pub desc: String,
    #[serde(default)]
    pub r#use: Option<String>,
    #[serde(default)]
    pub effect: Option<String>,
    #[serde(default)]
    pub r#type: String,
    #[serde(default)]
    pub schema: Option<String>,
    #[serde(default)]
    pub encrypted_data_key: Option<String>,
    #[serde(default)]
    pub src_user: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchPageParam {
    #[serde(flatten)]
    pub config_form: ConfigForm,
    pub page_no: u64,
    pub page_size: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteParam {
    pub data_id: String,
    pub group_name: String,
    pub tenant: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteForm {
    pub namespace_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    #[serde(default)]
    pub namespace_id: String,
    pub group: Option<String>,
    pub data_ids: Option<String>,
    pub app_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportRequest {
    #[serde(default)]
    pub namespace_id: String,
    pub policy: Option<String>,
}

impl ImportRequest {
    pub fn get_policy(&self) -> SameConfigPolicy {
        self.policy
            .as_ref()
            .map(|p| p.parse().unwrap_or_default())
            .unwrap_or_default()
    }
}

/// Config detail info for API response
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigDetailInfo {
    pub id: i64,
    pub data_id: String,
    pub group: String,
    pub content: String,
    pub md5: String,
    pub tenant: String,
    pub app_name: String,
    pub r#type: String,
    pub create_time: i64,
    pub modify_time: i64,
    pub create_user: String,
    pub create_ip: String,
    pub desc: String,
    pub r#use: String,
    pub effect: String,
    pub schema: String,
    pub config_tags: String,
    pub encrypted_data_key: String,
}

impl From<ConfigAllInfo> for ConfigDetailInfo {
    fn from(info: ConfigAllInfo) -> Self {
        Self {
            id: info.config_info.config_info_base.id,
            data_id: info.config_info.config_info_base.data_id,
            group: info.config_info.config_info_base.group,
            content: info.config_info.config_info_base.content,
            md5: info.config_info.config_info_base.md5,
            tenant: info.config_info.tenant,
            app_name: info.config_info.app_name,
            r#type: info.config_info.r#type,
            create_time: info.create_time,
            modify_time: info.modify_time,
            create_user: info.create_user,
            create_ip: info.create_ip,
            desc: info.desc,
            r#use: info.r#use,
            effect: info.effect,
            schema: info.schema,
            config_tags: info.config_tags,
            encrypted_data_key: info.config_info.config_info_base.encrypted_data_key,
        }
    }
}

/// Config gray info for API response
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigGrayInfo {
    pub id: i64,
    pub data_id: String,
    pub group: String,
    pub content: String,
    pub md5: String,
    pub tenant: String,
    pub gray_name: String,
    pub gray_rule: String,
    pub src_user: String,
    pub r#type: String,
}

impl From<ConfigInfoGrayWrapper> for ConfigGrayInfo {
    fn from(wrapper: ConfigInfoGrayWrapper) -> Self {
        Self {
            id: wrapper.config_info.config_info_base.id,
            data_id: wrapper.config_info.config_info_base.data_id,
            group: wrapper.config_info.config_info_base.group,
            content: wrapper.config_info.config_info_base.content,
            md5: wrapper.config_info.config_info_base.md5,
            tenant: wrapper.config_info.tenant,
            gray_name: wrapper.gray_name,
            gray_rule: wrapper.gray_rule,
            src_user: wrapper.src_user,
            r#type: wrapper.config_info.r#type,
        }
    }
}

#[get("")]
pub async fn find_one(
    datasource: web::Data<Arc<dyn ConsoleDataSource>>,
    params: web::Query<ConfigForm>,
) -> impl Responder {
    match datasource
        .config_get(&params.data_id, &params.group_name, &params.namespace_id)
        .await
    {
        Ok(config) => {
            let result = config.map(ConfigDetailInfo::from);
            ApiResult::<Option<ConfigDetailInfo>>::http_success(result)
        }
        Err(e) => ApiResult::http_internal_error(e),
    }
}

#[get("list")]
pub async fn search(
    datasource: web::Data<Arc<dyn ConsoleDataSource>>,
    params: web::Query<SearchPageParam>,
) -> impl Responder {
    let search_param = params.0;

    match datasource
        .config_list(
            search_param.page_no,
            search_param.page_size,
            &search_param.config_form.namespace_id,
            &search_param.config_form.data_id,
            &search_param.config_form.group_name,
            &search_param.config_form.app_name,
            &search_param.config_form.config_tags,
            &search_param.config_form.r#type,
            &search_param.config_form.content,
        )
        .await
    {
        Ok(page_result) => ApiResult::<Page<ConfigBasicInfo>>::http_success(page_result),
        Err(e) => ApiResult::http_internal_error(e),
    }
}

#[post("")]
pub async fn create_or_update(
    datasource: web::Data<Arc<dyn ConsoleDataSource>>,
    form: web::Form<ConfigForm>,
    src_user: Option<String>,
    src_ip: Option<String>,
) -> impl Responder {
    if form.data_id.is_empty() {
        return ApiResult::http_param_missing("dataId");
    }

    if form.group_name.is_empty() {
        return ApiResult::http_param_missing("groupName");
    }

    if form.content.is_empty() {
        return ApiResult::http_param_missing("content");
    }

    let mut config_form = form.into_inner();
    if config_form.namespace_id.is_empty() {
        config_form.namespace_id = DEFAULT_NAMESPACE_ID.to_string();
    }

    let user = src_user.unwrap_or_else(|| config_form.src_user.clone().unwrap_or_default());
    let ip = src_ip.unwrap_or_default();

    match datasource
        .config_publish(
            &config_form.data_id,
            &config_form.group_name,
            &config_form.namespace_id,
            &config_form.content,
            &config_form.app_name,
            &user,
            &ip,
            &config_form.config_tags,
            &config_form.desc,
            &config_form.r#use.unwrap_or_default(),
            &config_form.effect.unwrap_or_default(),
            &config_form.r#type,
            &config_form.schema.unwrap_or_default(),
            &config_form.encrypted_data_key.unwrap_or_default(),
        )
        .await
    {
        Ok(_) => ApiResult::<bool>::http_success(true),
        Err(e) => ApiResult::http_internal_error(e),
    }
}

#[delete("")]
pub async fn delete_config(
    datasource: web::Data<Arc<dyn ConsoleDataSource>>,
    params: web::Query<DeleteParam>,
    form: Option<web::Form<DeleteForm>>,
    src_user: Option<String>,
    src_ip: Option<String>,
) -> impl Responder {
    let mut tenant = params.tenant.to_string();

    if let Some(delete_form) = form {
        let namespace_id = delete_form.namespace_id.to_string();
        if !namespace_id.is_empty() && tenant.is_empty() {
            tenant = namespace_id;
        }
    }

    if tenant.is_empty() {
        tenant = DEFAULT_NAMESPACE_ID.to_string();
    }

    let user = src_user.unwrap_or_default();
    let ip = src_ip.unwrap_or_default();

    match datasource
        .config_delete(&params.data_id, &params.group_name, &tenant, "", &ip, &user)
        .await
    {
        Ok(_) => ApiResult::<bool>::http_success(true),
        Err(e) => ApiResult::http_internal_error(e),
    }
}

#[get("beta")]
pub async fn find_beta_one(
    datasource: web::Data<Arc<dyn ConsoleDataSource>>,
    params: web::Query<ConfigForm>,
) -> impl Responder {
    match datasource
        .config_gray_get(&params.data_id, &params.group_name, &params.namespace_id)
        .await
    {
        Ok(config) => {
            let result = config.map(ConfigGrayInfo::from);
            ApiResult::<Option<ConfigGrayInfo>>::http_success(result)
        }
        Err(e) => ApiResult::http_internal_error(e),
    }
}

#[get("export")]
pub async fn export_configs(
    datasource: web::Data<Arc<dyn ConsoleDataSource>>,
    params: web::Query<ExportRequest>,
) -> impl Responder {
    let namespace_id = if params.namespace_id.is_empty() {
        DEFAULT_NAMESPACE_ID.to_string()
    } else {
        params.namespace_id.clone()
    };

    match datasource
        .config_export(
            &namespace_id,
            params.group.as_deref(),
            params.data_ids.as_deref(),
            params.app_name.as_deref(),
        )
        .await
    {
        Ok(zip_data) => {
            let filename = format!(
                "nacos_config_export_{}.zip",
                Utc::now().format("%Y%m%d%H%M%S")
            );

            HttpResponse::Ok()
                .content_type("application/zip")
                .insert_header((
                    "Content-Disposition",
                    format!("attachment; filename=\"{}\"", filename),
                ))
                .body(zip_data)
        }
        Err(e) => ApiResult::http_internal_error(e),
    }
}

#[post("import")]
pub async fn import_configs(
    datasource: web::Data<Arc<dyn ConsoleDataSource>>,
    params: web::Query<ImportRequest>,
    mut payload: Multipart,
    src_user: Option<String>,
    src_ip: Option<String>,
) -> impl Responder {
    let namespace_id = if params.namespace_id.is_empty() {
        DEFAULT_NAMESPACE_ID.to_string()
    } else {
        params.namespace_id.clone()
    };

    let policy = params.get_policy();

    let user = src_user.unwrap_or_default();
    let ip = src_ip.unwrap_or_default();

    // Read file from multipart with proper error handling
    let mut file_data: Vec<u8> = Vec::new();
    while let Some(field_result) = payload.next().await {
        let mut field = match field_result {
            Ok(f) => f,
            Err(e) => {
                warn!(error = %e, "Failed to read multipart field");
                return ApiResult::http_internal_error(e.to_string());
            }
        };

        if let Some(content_disposition) = field.content_disposition()
            && content_disposition.get_name().is_some_and(|n| n == "file")
        {
            while let Some(chunk_result) = field.next().await {
                match chunk_result {
                    Ok(chunk) => file_data.extend_from_slice(&chunk),
                    Err(e) => {
                        warn!(error = %e, "Failed to read multipart chunk");
                        return ApiResult::http_internal_error(e.to_string());
                    }
                }
            }
            break;
        }
    }

    if file_data.is_empty() {
        return ApiResult::http_param_missing("file");
    }

    match datasource
        .config_import(file_data, &namespace_id, policy, &user, &ip)
        .await
    {
        Ok(result) => ApiResult::http_success(result),
        Err(e) => ApiResult::http_internal_error(e),
    }
}

pub fn routes() -> Scope {
    web::scope("/cs/config")
        .service(find_one)
        .service(search)
        .service(create_or_update)
        .service(delete_config)
        .service(find_beta_one)
        .service(export_configs)
        .service(import_configs)
}
