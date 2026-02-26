//! V3 Client config SDK API endpoint

use actix_web::{HttpMessage, HttpRequest, Responder, get, web};
use tracing::warn;

use crate::{
    ActionTypes, ApiType, Secured, SignType, error, model::common::AppState,
    model::response::Result, secured,
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

    let persistence = data.persistence();
    match persistence
        .config_find_one(&params.data_id, &params.group, namespace_id)
        .await
    {
        Ok(Some(config)) => {
            let response = ConfigResponse {
                id: String::new(),
                data_id: config.data_id.clone(),
                group: config.group.clone(),
                content: config.content.clone(),
                md5: config.md5.clone(),
                encrypted_data_key: if config.encrypted_data_key.is_empty() {
                    None
                } else {
                    Some(config.encrypted_data_key.clone())
                },
                tenant: config.tenant.clone(),
                app_name: if config.app_name.is_empty() {
                    None
                } else {
                    Some(config.app_name.clone())
                },
                r#type: if config.config_type.is_empty() {
                    None
                } else {
                    Some(config.config_type.clone())
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
            Result::<String>::http_response(
                500,
                error::SERVER_ERROR.code,
                e.to_string(),
                String::new(),
            )
        }
    }
}

pub fn routes() -> actix_web::Scope {
    web::scope("/config").service(get_config)
}
