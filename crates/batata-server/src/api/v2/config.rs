//! V2 Config API handlers
//!
//! Implements the Nacos V2 configuration management API endpoints:
//! - GET /nacos/v2/cs/config - Get config
//! - POST /nacos/v2/cs/config - Publish config
//! - DELETE /nacos/v2/cs/config - Delete config

use actix_web::{HttpMessage, HttpRequest, Responder, delete, get, post, web};
use tracing::{info, warn};

use crate::{
    ActionTypes, ApiType, Secured, SignType, model::common::AppState, model::response::Result,
    secured,
};

use super::model::{ConfigDeleteParam, ConfigGetParam, ConfigPublishParam, ConfigResponse};

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
            400,
            "Required parameter 'dataId' is missing".to_string(),
            String::new(),
        );
    }

    if params.group.is_empty() {
        return Result::<String>::http_response(
            400,
            400,
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

    // Get config from service
    let db = data.db();
    match batata_config::service::config::find_one(db, &params.data_id, &params.group, namespace_id)
        .await
    {
        Ok(Some(config)) => {
            let response = ConfigResponse {
                id: config.config_info.config_info_base.id.to_string(),
                data_id: config.config_info.config_info_base.data_id,
                group: config.config_info.config_info_base.group,
                content: config.config_info.config_info_base.content,
                md5: config.config_info.config_info_base.md5,
                encrypted_data_key: if config
                    .config_info
                    .config_info_base
                    .encrypted_data_key
                    .is_empty()
                {
                    None
                } else {
                    Some(config.config_info.config_info_base.encrypted_data_key)
                },
                tenant: config.config_info.tenant,
                app_name: if config.config_info.app_name.is_empty() {
                    None
                } else {
                    Some(config.config_info.app_name)
                },
                r#type: if config.config_info.r#type.is_empty() {
                    None
                } else {
                    Some(config.config_info.r#type)
                },
            };
            Result::<ConfigResponse>::http_success(response)
        }
        Ok(None) => Result::<Option<ConfigResponse>>::http_response(
            404,
            404,
            format!(
                "config data not exist, dataId={}, group={}, tenant={}",
                params.data_id, params.group, namespace_id
            ),
            None::<ConfigResponse>,
        ),
        Err(e) => {
            warn!(error = %e, "Failed to get config");
            Result::<String>::http_response(500, 500, e.to_string(), String::new())
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
        return Result::<bool>::http_response(
            400,
            400,
            "Required parameter 'dataId' is missing".to_string(),
            false,
        );
    }

    if form.group.is_empty() {
        return Result::<bool>::http_response(
            400,
            400,
            "Required parameter 'group' is missing".to_string(),
            false,
        );
    }

    if form.content.is_empty() {
        return Result::<bool>::http_response(
            400,
            400,
            "Required parameter 'content' is missing".to_string(),
            false,
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

    // Publish config using service
    let db = data.db();
    match batata_config::service::config::create_or_update(
        db,
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
            Result::<bool>::http_success(true)
        }
        Err(e) => {
            warn!(error = %e, "Failed to publish config");
            Result::<bool>::http_response(500, 500, e.to_string(), false)
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
        return Result::<bool>::http_response(
            400,
            400,
            "Required parameter 'dataId' is missing".to_string(),
            false,
        );
    }

    if params.group.is_empty() {
        return Result::<bool>::http_response(
            400,
            400,
            "Required parameter 'group' is missing".to_string(),
            false,
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

    // Delete config using service
    let db = data.db();
    match batata_config::service::config::delete(
        db,
        &params.data_id,
        &params.group,
        namespace_id,
        "",
        &src_ip,
        "",
    )
    .await
    {
        Ok(deleted) => {
            if deleted {
                info!(
                    data_id = %params.data_id,
                    group = %params.group,
                    namespace_id = %namespace_id,
                    "Config deleted successfully"
                );
                Result::<bool>::http_success(true)
            } else {
                Result::<bool>::http_response(
                    404,
                    404,
                    format!(
                        "config data not exist, dataId={}, group={}, tenant={}",
                        params.data_id, params.group, namespace_id
                    ),
                    false,
                )
            }
        }
        Err(e) => {
            warn!(error = %e, "Failed to delete config");
            Result::<bool>::http_response(500, 500, e.to_string(), false)
        }
    }
}
