//! V3 Admin config management endpoints

use std::str::FromStr;

use actix_multipart::Multipart;
use actix_web::{
    HttpMessage, HttpRequest, HttpResponse, Responder, delete, get, http::StatusCode, post, put,
    web,
};
use chrono::Utc;
use futures::StreamExt;
use serde::Deserialize;

use crate::{
    ActionTypes, ApiType, Secured, SignType,
    api::config::model::{ConfigBasicInfo, ConfigDetailInfo},
    auth::model::AuthContext,
    config::{
        export_model::{ExportRequest, ImportRequest, ImportResult},
        model::{ConfigForm, ConfigType},
    },
    error, is_valid,
    model::{
        self,
        common::{AppState, DEFAULT_NAMESPACE_ID},
    },
    secured, service,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct SearchPageParam {
    #[serde(flatten)]
    config_form: ConfigForm,
    pub page_no: u64,
    pub page_size: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListConfigParam {
    #[serde(flatten)]
    config_form: ConfigForm,
    #[serde(default = "default_page_no")]
    pub page_no: u64,
    #[serde(default = "default_page_size")]
    pub page_size: u64,
    #[serde(default = "default_search")]
    pub search: String,
    #[serde(default)]
    pub config_detail: String,
}

fn default_page_no() -> u64 {
    1
}

fn default_page_size() -> u64 {
    20
}

fn default_search() -> String {
    "blur".to_string()
}

/// GET /v3/admin/cs/config
#[get("")]
async fn get_config(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<ConfigForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let result = match data
        .persistence()
        .config_find_one(&params.data_id, &params.group_name, &params.namespace_id)
        .await
    {
        Ok(config) => config.map(ConfigDetailInfo::from),
        Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
    };

    model::common::Result::<Option<ConfigDetailInfo>>::http_success(result)
}

/// GET /v3/admin/cs/config/list
#[get("list")]
async fn list_configs(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<ListConfigParam>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let mut namespace_id = params.config_form.namespace_id.clone();
    if namespace_id.is_empty() {
        namespace_id = DEFAULT_NAMESPACE_ID.to_string();
    }

    let tags: Vec<String> = if !params.config_form.config_tags.is_empty() {
        params
            .config_form
            .config_tags
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    } else {
        Vec::new()
    };

    let types: Vec<String> = if !params.config_form.r#type.is_empty() {
        params
            .config_form
            .r#type
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    } else {
        Vec::new()
    };

    match data
        .persistence()
        .config_search_page(
            params.page_no,
            params.page_size,
            &namespace_id,
            &params.config_form.data_id,
            &params.config_form.group_name,
            &params.config_form.app_name,
            tags,
            types,
            &params.config_detail,
        )
        .await
    {
        Ok(page) => {
            let api_page = batata_api::Page::<ConfigBasicInfo>::new(
                page.total_count,
                page.page_number,
                params.page_size,
                page.page_items
                    .into_iter()
                    .map(ConfigBasicInfo::from)
                    .collect(),
            );
            model::common::Result::<batata_api::Page<ConfigBasicInfo>>::http_success(api_page)
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to list configs");
            HttpResponse::InternalServerError().body(e.to_string())
        }
    }
}

/// POST /v3/admin/cs/config
#[post("")]
async fn create_config(
    req: HttpRequest,
    data: web::Data<AppState>,
    form: web::Form<ConfigForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    if form.data_id.is_empty() {
        return model::common::Result::<String>::http_response(
            StatusCode::BAD_REQUEST.as_u16(),
            error::PARAMETER_MISSING.code,
            error::PARAMETER_MISSING.message.to_string(),
            "Required parameter 'dataId' type String is not present",
        );
    }

    if !is_valid(&form.data_id) {
        return model::common::Result::<String>::http_response(
            StatusCode::BAD_REQUEST.as_u16(),
            error::PARAMETER_VALIDATE_ERROR.code,
            error::PARAMETER_VALIDATE_ERROR.message.to_string(),
            format!("invalid dataId : {}", form.data_id),
        );
    }

    if form.group_name.is_empty() {
        return model::common::Result::<String>::http_response(
            StatusCode::BAD_REQUEST.as_u16(),
            error::PARAMETER_MISSING.code,
            error::PARAMETER_MISSING.message.to_string(),
            "Required parameter 'groupName' type String is not present",
        );
    }

    if !is_valid(&form.group_name) {
        return model::common::Result::<String>::http_response(
            StatusCode::BAD_REQUEST.as_u16(),
            error::PARAMETER_VALIDATE_ERROR.code,
            error::PARAMETER_VALIDATE_ERROR.message.to_string(),
            format!("invalid group : {}", form.group_name),
        );
    }

    if form.content.is_empty() {
        return model::common::Result::<String>::http_response(
            StatusCode::BAD_REQUEST.as_u16(),
            error::PARAMETER_MISSING.code,
            error::PARAMETER_MISSING.message.to_string(),
            "Required parameter 'content' type String is not present",
        );
    }

    if form.content.chars().count() > data.configuration.max_content() as usize {
        return model::common::Result::<String>::http_response(
            StatusCode::BAD_REQUEST.as_u16(),
            error::PARAMETER_VALIDATE_ERROR.code,
            error::PARAMETER_VALIDATE_ERROR.message.to_string(),
            format!("invalid content, over {}", data.configuration.max_content()),
        );
    }

    let mut config_form = form.into_inner();
    if config_form.namespace_id.is_empty() {
        config_form.namespace_id = DEFAULT_NAMESPACE_ID.to_string();
    }
    config_form.namespace_id = config_form.namespace_id.trim().to_string();
    config_form.r#type = ConfigType::from_str(&config_form.r#type)
        .unwrap_or_default()
        .to_string();

    let src_user = match req.extensions().get::<AuthContext>() {
        Some(ctx) => config_form
            .src_user
            .take()
            .unwrap_or_else(|| ctx.username.clone()),
        None => config_form.src_user.take().unwrap_or_default(),
    };

    let src_ip = req
        .connection_info()
        .realip_remote_addr()
        .unwrap_or_default()
        .to_owned();

    let _ = data
        .persistence()
        .config_create_or_update(
            &config_form.data_id,
            &config_form.group_name,
            &config_form.namespace_id,
            &config_form.content,
            &config_form.app_name,
            &src_user,
            &src_ip,
            &config_form.config_tags,
            &config_form.desc,
            &config_form.r#use.unwrap_or_default(),
            &config_form.effect.unwrap_or_default(),
            &config_form.r#type,
            &config_form.schema.unwrap_or_default(),
            &config_form.encrypted_data_key.unwrap_or_default(),
        )
        .await;

    model::common::Result::<bool>::http_success(true)
}

/// PUT /v3/admin/cs/config
#[put("")]
async fn update_config(
    req: HttpRequest,
    data: web::Data<AppState>,
    form: web::Form<ConfigForm>,
) -> impl Responder {
    // Same as create - Nacos create_or_update semantics
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    if form.data_id.is_empty() || form.group_name.is_empty() || form.content.is_empty() {
        return model::common::Result::<String>::http_response(
            StatusCode::BAD_REQUEST.as_u16(),
            error::PARAMETER_MISSING.code,
            error::PARAMETER_MISSING.message.to_string(),
            "Required parameters 'dataId', 'groupName', 'content' are missing",
        );
    }

    let mut config_form = form.into_inner();
    if config_form.namespace_id.is_empty() {
        config_form.namespace_id = DEFAULT_NAMESPACE_ID.to_string();
    }
    config_form.namespace_id = config_form.namespace_id.trim().to_string();
    config_form.r#type = ConfigType::from_str(&config_form.r#type)
        .unwrap_or_default()
        .to_string();

    let src_user = match req.extensions().get::<AuthContext>() {
        Some(ctx) => config_form
            .src_user
            .take()
            .unwrap_or_else(|| ctx.username.clone()),
        None => config_form.src_user.take().unwrap_or_default(),
    };
    let src_ip = req
        .connection_info()
        .realip_remote_addr()
        .unwrap_or_default()
        .to_owned();

    let _ = data
        .persistence()
        .config_create_or_update(
            &config_form.data_id,
            &config_form.group_name,
            &config_form.namespace_id,
            &config_form.content,
            &config_form.app_name,
            &src_user,
            &src_ip,
            &config_form.config_tags,
            &config_form.desc,
            &config_form.r#use.unwrap_or_default(),
            &config_form.effect.unwrap_or_default(),
            &config_form.r#type,
            &config_form.schema.unwrap_or_default(),
            &config_form.encrypted_data_key.unwrap_or_default(),
        )
        .await;

    model::common::Result::<bool>::http_success(true)
}

/// DELETE /v3/admin/cs/config
#[delete("")]
async fn delete_config(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<ConfigForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let namespace_id = if params.namespace_id.is_empty() {
        DEFAULT_NAMESPACE_ID.to_string()
    } else {
        params.namespace_id.clone()
    };

    let client_ip = req
        .connection_info()
        .realip_remote_addr()
        .unwrap_or_default()
        .to_owned();

    let src_user = req
        .extensions()
        .get::<AuthContext>()
        .map(|ctx| ctx.username.clone())
        .unwrap_or_default();

    if let Err(e) = data
        .persistence()
        .config_delete(
            &params.data_id,
            &params.group_name,
            &namespace_id,
            "",
            &client_ip,
            &src_user,
        )
        .await
    {
        return HttpResponse::InternalServerError().body(e.to_string());
    }

    model::common::Result::<bool>::http_success(true)
}

/// POST /v3/admin/cs/config/import
#[post("import")]
async fn import_config(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<ImportRequest>,
    mut payload: Multipart,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let namespace_id = if params.namespace_id.is_empty() {
        DEFAULT_NAMESPACE_ID.to_string()
    } else {
        params.namespace_id.clone()
    };

    let policy = params.get_policy();

    let src_user = req
        .extensions()
        .get::<AuthContext>()
        .map(|ctx| ctx.username.clone())
        .unwrap_or_default();
    let src_ip = req
        .connection_info()
        .realip_remote_addr()
        .unwrap_or_default()
        .to_owned();

    let mut file_data: Vec<u8> = Vec::new();
    while let Some(Ok(mut field)) = payload.next().await {
        if let Some(content_disposition) = field.content_disposition()
            && content_disposition
                .get_name()
                .map(|n| n == "file")
                .unwrap_or(false)
        {
            while let Some(Ok(chunk)) = field.next().await {
                file_data.extend_from_slice(&chunk);
            }
            break;
        }
    }

    if file_data.is_empty() {
        return model::common::Result::<ImportResult>::http_response(
            StatusCode::BAD_REQUEST.as_u16(),
            error::PARAMETER_MISSING.code,
            error::PARAMETER_MISSING.message.to_string(),
            "No file uploaded",
        );
    }

    let items = match service::config_import::parse_nacos_import_zip(&file_data) {
        Ok(i) => i,
        Err(e) => {
            return model::common::Result::<ImportResult>::http_response(
                StatusCode::BAD_REQUEST.as_u16(),
                error::PARAMETER_VALIDATE_ERROR.code,
                error::PARAMETER_VALIDATE_ERROR.message.to_string(),
                format!("Invalid ZIP file: {}", e),
            );
        }
    };

    if items.is_empty() {
        return model::common::Result::<ImportResult>::http_response(
            StatusCode::BAD_REQUEST.as_u16(),
            error::DATA_EMPTY.code,
            error::DATA_EMPTY.message.to_string(),
            "No configurations found in ZIP file",
        );
    }

    let persistence = data.persistence();
    let config_items: Vec<_> = items.into_iter().map(|i| i.into()).collect();
    let result = match import_with_persistence(
        persistence,
        config_items,
        &namespace_id,
        policy,
        &src_user,
        &src_ip,
    )
    .await
    {
        Ok(r) => r,
        Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
    };

    model::common::Result::<ImportResult>::http_success(result)
}

/// GET /v3/admin/cs/config/export
#[get("export")]
async fn export_config(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<ExportRequest>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let namespace_id = if params.namespace_id.is_empty() {
        DEFAULT_NAMESPACE_ID.to_string()
    } else {
        params.namespace_id.clone()
    };

    let data_ids = params.data_ids.as_ref().map(|ids| {
        ids.split(',')
            .filter(|s| !s.is_empty())
            .map(|s| s.trim().to_string())
            .collect::<Vec<String>>()
    });

    let persistence = data.persistence();
    let storage_configs = match persistence
        .config_find_for_export(
            &namespace_id,
            params.group.as_deref(),
            data_ids,
            params.app_name.as_deref(),
        )
        .await
    {
        Ok(c) => c,
        Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
    };

    if storage_configs.is_empty() {
        return HttpResponse::NotFound().body("No configurations found to export");
    }

    // Convert to ConfigAllInfo for the existing export function
    let configs: Vec<batata_config::model::ConfigAllInfo> =
        storage_configs.into_iter().map(Into::into).collect();

    let zip_data = match service::config_export::create_nacos_export_zip(configs) {
        Ok(z) => z,
        Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
    };

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

/// Import configs using the persistence service instead of direct DB access
async fn import_with_persistence(
    persistence: &dyn batata_persistence::PersistenceService,
    items: Vec<crate::config::export_model::ConfigImportItem>,
    target_namespace_id: &str,
    policy: crate::config::export_model::SameConfigPolicy,
    src_user: &str,
    src_ip: &str,
) -> anyhow::Result<ImportResult> {
    use crate::config::export_model::{ImportFailItem, SameConfigPolicy};

    let mut result = ImportResult::default();

    for item in items {
        let namespace_id = if item.namespace_id.is_empty() {
            target_namespace_id.to_string()
        } else {
            item.namespace_id.clone()
        };

        // Check if config already exists
        let exists = persistence
            .config_find_one(&item.data_id, &item.group, &namespace_id)
            .await?
            .is_some();

        if exists {
            match policy {
                SameConfigPolicy::Abort => {
                    result.fail_count += 1;
                    result.fail_data.push(ImportFailItem {
                        data_id: item.data_id.clone(),
                        group: item.group.clone(),
                        reason: "Configuration already exists".to_string(),
                    });
                    return Ok(result);
                }
                SameConfigPolicy::Skip => {
                    result.skip_count += 1;
                    continue;
                }
                SameConfigPolicy::Overwrite => {
                    // Continue to update
                }
            }
        }

        // Create or update the configuration
        match persistence
            .config_create_or_update(
                &item.data_id,
                &item.group,
                &namespace_id,
                &item.content,
                &item.app_name,
                src_user,
                src_ip,
                &item.config_tags,
                &item.desc,
                "",
                "",
                &item.config_type,
                "",
                &item.encrypted_data_key,
            )
            .await
        {
            Ok(_) => {
                result.success_count += 1;
            }
            Err(e) => {
                result.fail_count += 1;
                result.fail_data.push(ImportFailItem {
                    data_id: item.data_id.clone(),
                    group: item.group.clone(),
                    reason: e.to_string(),
                });
            }
        }
    }

    Ok(result)
}

pub fn routes() -> actix_web::Scope {
    web::scope("/config")
        .service(get_config)
        .service(list_configs)
        .service(create_config)
        .service(update_config)
        .service(delete_config)
        .service(import_config)
        .service(export_config)
}
