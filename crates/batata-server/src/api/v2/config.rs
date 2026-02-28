//! V2 Config API handlers
//!
//! Implements the Nacos V2 configuration management API endpoints:
//! - GET /nacos/v2/cs/config - Get config
//! - POST /nacos/v2/cs/config - Publish config
//! - DELETE /nacos/v2/cs/config - Delete config
//! - GET /nacos/v2/cs/config/searchDetail - Search config detail

use actix_web::{HttpMessage, HttpRequest, HttpResponse, Responder, delete, get, post, web};
use batata_api::model::Page;
use batata_config::model::config::ConfigBasicInfo;
use tracing::{info, warn};

use crate::{
    ActionTypes, ApiType, Secured, SignType, error, model::common::AppState,
    model::response::Result, secured,
};

use super::model::{
    ConfigDeleteParam, ConfigGetParam, ConfigPublishParam, ConfigSearchDetailParam,
};

/// Helper to convert ConfigStorageData to ConfigBasicInfo
fn config_storage_to_basic_info(config: batata_persistence::ConfigStorageData) -> ConfigBasicInfo {
    ConfigBasicInfo {
        id: 0,
        namespace_id: config.tenant,
        group_name: config.group,
        data_id: config.data_id,
        md5: config.md5,
        r#type: config.config_type,
        app_name: config.app_name,
        create_time: config.created_time,
        modify_time: config.modified_time,
    }
}

/// Get configuration
///
/// GET /nacos/v2/cs/config
///
/// Retrieves a configuration by dataId, group, and optional namespaceId.
#[get("")]
pub async fn get_config(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<ConfigGetParam>,
) -> impl Responder {
    // Validate required parameters
    if params.data_id.is_empty() {
        return Result::<String>::http_response(
            400,
            error::PARAMETER_MISSING.code,
            "Required parameter 'dataId' is missing".to_string(),
            String::new(),
        );
    }

    if params.group.is_empty() {
        return Result::<String>::http_response(
            400,
            error::PARAMETER_MISSING.code,
            "Required parameter 'group' is missing".to_string(),
            String::new(),
        );
    }

    let namespace_id = params.namespace_id_or_default();

    // Check authorization
    let resource = format!(
        "{}:{}:config/{}",
        namespace_id, params.group, params.data_id
    );
    secured!(
        Secured::builder(&req, &data, &resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::OpenApi)
            .build()
    );

    // Get config from persistence service
    let persistence = data.persistence();

    // Check for matching gray config by client IP
    let client_ip = req
        .connection_info()
        .realip_remote_addr()
        .unwrap_or("unknown")
        .to_string();
    let mut labels = std::collections::HashMap::new();
    labels.insert(
        batata_config::model::gray_rule::labels::CLIENT_IP.to_string(),
        client_ip,
    );

    if let Ok(grays) = persistence
        .config_find_all_grays(&params.data_id, &params.group, namespace_id)
        .await
    {
        // Parse and sort by priority (higher priority first)
        let mut candidates: Vec<_> = grays
            .iter()
            .filter_map(|gray| {
                let rule = batata_config::model::gray_rule::parse_gray_rule(&gray.gray_rule)?;
                Some((gray, rule))
            })
            .collect();
        candidates.sort_by(|a, b| b.1.priority().cmp(&a.1.priority()));

        for (gray, rule) in candidates {
            if rule.matches(&labels) {
                return HttpResponse::Ok()
                    .insert_header(("Config-Type", ""))
                    .insert_header(("Content-MD5", gray.md5.as_str()))
                    .insert_header(("Cache-Control", "no-cache"))
                    .insert_header(("Pragma", "no-cache"))
                    .json(Result::<String>::success(gray.content.clone()));
            }
        }
    }

    // Fall through to formal config query
    match persistence
        .config_find_one(&params.data_id, &params.group, namespace_id)
        .await
    {
        Ok(Some(config)) => HttpResponse::Ok()
            .insert_header(("Config-Type", config.config_type.as_str()))
            .insert_header(("Content-MD5", config.md5.as_str()))
            .insert_header(("Cache-Control", "no-cache"))
            .insert_header(("Pragma", "no-cache"))
            .json(Result::<String>::success(config.content)),
        Ok(None) => Result::<Option<String>>::http_response(
            404,
            error::RESOURCE_NOT_FOUND.code,
            format!(
                "config data not exist, dataId={}, group={}, tenant={}",
                params.data_id, params.group, namespace_id
            ),
            None::<String>,
        ),
        Err(e) => {
            warn!(error = %e, "Failed to get config");
            Result::<String>::http_response(
                500,
                error::SERVER_ERROR.code,
                e.to_string(),
                String::new(),
            )
        }
    }
}

/// Publish configuration
///
/// POST /nacos/v2/cs/config
///
/// Creates or updates a configuration.
#[post("")]
pub async fn publish_config(
    req: HttpRequest,
    data: web::Data<AppState>,
    form: web::Form<ConfigPublishParam>,
) -> impl Responder {
    // Validate required parameters
    if form.data_id.is_empty() {
        return Result::<String>::http_response(
            400,
            error::PARAMETER_MISSING.code,
            "Required parameter 'dataId' is missing".to_string(),
            String::new(),
        );
    }

    if form.group.is_empty() {
        return Result::<String>::http_response(
            400,
            error::PARAMETER_MISSING.code,
            "Required parameter 'group' is missing".to_string(),
            String::new(),
        );
    }

    if form.content.is_empty() {
        return Result::<String>::http_response(
            400,
            error::PARAMETER_MISSING.code,
            "Required parameter 'content' is missing".to_string(),
            String::new(),
        );
    }

    let namespace_id = form.namespace_id_or_default();

    // Check authorization
    let resource = format!("{}:{}:config/{}", namespace_id, form.group, form.data_id);
    secured!(
        Secured::builder(&req, &data, &resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Config)
            .api_type(ApiType::OpenApi)
            .build()
    );

    // Extract client IP from request
    let src_ip = req
        .connection_info()
        .realip_remote_addr()
        .unwrap_or("unknown")
        .to_string();

    let src_user = form.src_user.clone().unwrap_or_default();

    // Read betaIps from request header instead of form body
    let beta_ips = req
        .headers()
        .get("betaIps")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let persistence = data.persistence();

    // Handle gray (beta) publish if betaIps is provided
    if let Some(ref beta_ips) = beta_ips
        && !beta_ips.is_empty()
    {
        let gray_rule_info = batata_config::model::gray_rule::GrayRulePersistInfo::new_beta(
            beta_ips,
            batata_config::model::gray_rule::BetaGrayRule::PRIORITY,
        );
        let gray_rule = match gray_rule_info.to_json() {
            Ok(json) => json,
            Err(e) => {
                return Result::<String>::http_response(
                    500,
                    error::SERVER_ERROR.code,
                    format!("Failed to serialize gray rule: {}", e),
                    String::new(),
                );
            }
        };

        match persistence
            .config_create_or_update_gray(
                &form.data_id,
                &form.group,
                namespace_id,
                &form.content,
                "beta",
                &gray_rule,
                &src_user,
                &src_ip,
                form.app_name.as_deref().unwrap_or(""),
                "",
            )
            .await
        {
            Ok(_) => {
                info!(
                    data_id = %form.data_id,
                    group = %form.group,
                    namespace_id = %namespace_id,
                    "Beta config published successfully"
                );
                return Result::<String>::http_success("ok".to_string());
            }
            Err(e) => {
                warn!(error = %e, "Failed to publish beta config");
                return Result::<String>::http_response(
                    500,
                    error::SERVER_ERROR.code,
                    e.to_string(),
                    String::new(),
                );
            }
        }
    }

    // Normal (formal) config publish
    match persistence
        .config_create_or_update(
            &form.data_id,
            &form.group,
            namespace_id,
            &form.content,
            form.app_name.as_deref().unwrap_or(""),
            &src_user,
            &src_ip,
            form.config_tags.as_deref().unwrap_or(""),
            form.desc.as_deref().unwrap_or(""),
            form.r#use.as_deref().unwrap_or(""),
            form.effect.as_deref().unwrap_or(""),
            form.r#type.as_deref().unwrap_or(""),
            form.schema.as_deref().unwrap_or(""),
            "",
        )
        .await
    {
        Ok(_) => {
            info!(
                data_id = %form.data_id,
                group = %form.group,
                namespace_id = %namespace_id,
                "Config published successfully"
            );
            Result::<String>::http_success("ok".to_string())
        }
        Err(e) => {
            warn!(error = %e, "Failed to publish config");
            Result::<String>::http_response(
                500,
                error::SERVER_ERROR.code,
                e.to_string(),
                String::new(),
            )
        }
    }
}

/// Delete configuration
///
/// DELETE /nacos/v2/cs/config
///
/// Deletes a configuration by dataId, group, and optional namespaceId.
#[delete("")]
pub async fn delete_config(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<ConfigDeleteParam>,
) -> impl Responder {
    // Validate required parameters
    if params.data_id.is_empty() {
        return Result::<String>::http_response(
            400,
            error::PARAMETER_MISSING.code,
            "Required parameter 'dataId' is missing".to_string(),
            String::new(),
        );
    }

    if params.group.is_empty() {
        return Result::<String>::http_response(
            400,
            error::PARAMETER_MISSING.code,
            "Required parameter 'group' is missing".to_string(),
            String::new(),
        );
    }

    let namespace_id = params.namespace_id_or_default();

    // Check authorization
    let resource = format!(
        "{}:{}:config/{}",
        namespace_id, params.group, params.data_id
    );
    secured!(
        Secured::builder(&req, &data, &resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Config)
            .api_type(ApiType::OpenApi)
            .build()
    );

    // Extract client IP from request
    let src_ip = req
        .connection_info()
        .realip_remote_addr()
        .unwrap_or("unknown")
        .to_string();

    // Delete config using persistence service
    let persistence = data.persistence();
    match persistence
        .config_delete(
            &params.data_id,
            &params.group,
            namespace_id,
            "",
            &src_ip,
            "",
        )
        .await
    {
        Ok(_) => {
            info!(
                data_id = %params.data_id,
                group = %params.group,
                namespace_id = %namespace_id,
                "Config deleted successfully"
            );
            Result::<String>::http_success("ok".to_string())
        }
        Err(e) => {
            warn!(error = %e, "Failed to delete config");
            Result::<String>::http_response(
                500,
                error::SERVER_ERROR.code,
                e.to_string(),
                String::new(),
            )
        }
    }
}

/// Search config detail
///
/// GET /nacos/v2/cs/config/searchDetail
///
/// Searches configs with pagination and filtering.
#[get("searchDetail")]
pub async fn search_config_detail(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<ConfigSearchDetailParam>,
) -> impl Responder {
    let namespace_id = params.namespace_id_or_default();

    // Check authorization
    let resource = format!("{}:*:config/*", namespace_id);
    secured!(
        Secured::builder(&req, &data, &resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::OpenApi)
            .build()
    );

    let tags: Vec<String> = params
        .config_tags
        .as_deref()
        .unwrap_or("")
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    let types: Vec<String> = params
        .config_type
        .as_deref()
        .unwrap_or("")
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    let persistence = data.persistence();
    match persistence
        .config_search_page(
            params.page_no,
            params.page_size,
            namespace_id,
            params.data_id.as_deref().unwrap_or(""),
            params.group.as_deref().unwrap_or(""),
            params.app_name.as_deref().unwrap_or(""),
            tags,
            types,
            params.content.as_deref().unwrap_or(""),
        )
        .await
    {
        Ok(page) => {
            // Convert Page<ConfigStorageData> to Page<ConfigBasicInfo>
            let result_page = Page {
                total_count: page.total_count,
                page_number: page.page_number,
                pages_available: page.pages_available,
                page_items: page
                    .page_items
                    .into_iter()
                    .map(config_storage_to_basic_info)
                    .collect(),
            };
            Result::<Page<ConfigBasicInfo>>::http_success(result_page)
        }
        Err(e) => {
            warn!(error = %e, "Failed to search config detail");
            Result::<Page<ConfigBasicInfo>>::http_response(
                500,
                error::SERVER_ERROR.code,
                e.to_string(),
                Page::<ConfigBasicInfo>::default(),
            )
        }
    }
}
