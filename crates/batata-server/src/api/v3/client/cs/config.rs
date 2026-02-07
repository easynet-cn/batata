//! V3 Client config SDK API endpoint

use actix_web::{HttpMessage, HttpRequest, Responder, get, web};
use tracing::warn;

use crate::{
    ActionTypes, ApiType, Secured, SignType, model::common::AppState, model::response::Result,
    secured,
};

use crate::api::v2::model::{ConfigGetParam, ConfigResponse};

/// GET /v3/client/cs/config
#[get("")]
async fn get_config(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<ConfigGetParam>,
) -> impl Responder {
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

pub fn routes() -> actix_web::Scope {
    web::scope("/config").service(get_config)
}
