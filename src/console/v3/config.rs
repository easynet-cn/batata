use std::collections::HashMap;

use actix_web::{
    HttpMessage, HttpRequest, HttpResponse, Responder, Scope, delete, get, http::StatusCode, post,
    web,
};
use serde::Deserialize;

use chrono::Utc;

use crate::{
    ActionTypes, ApiType, Secured, SignType,
    api::{
        config::model::{ConfigBasicInfo, ConfigDetailInfo, ConfigGrayInfo, ConfigListenerInfo},
        model::Page,
    },
    auth::model::AuthContext,
    config::model::{ConfigAllInfo, ConfigForm, ConfigType},
    error, is_valid,
    model::{
        self,
        common::{AppState, DEFAULT_NAMESPACE_ID, ErrorResult},
    },
    secured, service,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchPageParam {
    #[serde(flatten)]
    config_form: ConfigForm,
    pub page_no: u64,
    pub page_size: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteParam {
    pub data_id: String,
    pub group_name: String,
    pub tenant: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteForm {
    pub namespace_id: String,
}

#[get("")]
async fn find_one(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<ConfigForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let result = service::config::find_one(
        &data.database_connection,
        &params.data_id,
        &params.group_name,
        &params.namespace_id,
    )
    .await
    .unwrap()
    .map(ConfigDetailInfo::from);

    model::common::Result::<Option<ConfigAllInfo>>::http_success(result)
}

#[get("list")]
async fn search(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<SearchPageParam>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let search_param = params.0;
    let tags = search_param
        .config_form
        .config_tags
        .split(",")
        .into_iter()
        .filter(|e| !e.is_empty())
        .map(|e| e.to_string())
        .collect::<Vec<String>>();
    let types = search_param
        .config_form
        .r#type
        .split(",")
        .into_iter()
        .filter(|e| !e.is_empty())
        .map(|e| e.to_string())
        .collect::<Vec<String>>();

    let result = crate::service::config::search_page(
        &data.database_connection,
        search_param.page_no,
        search_param.page_size,
        &search_param.config_form.namespace_id,
        &search_param.config_form.data_id,
        &search_param.config_form.group_name,
        &search_param.config_form.app_name,
        tags,
        types,
        &search_param.config_form.content,
    )
    .await;

    return match result {
        Ok(page_result) => {
            model::common::Result::<Page<ConfigBasicInfo>>::http_success(page_result)
        }
        Err(err) => HttpResponse::InternalServerError().json(ErrorResult {
            timestamp: Utc::now().to_rfc3339(),
            status: 403,
            message: err.to_string(),
            error: String::from("Forbiden"),
            path: req.path().to_string(),
        }),
    };
}

#[post("")]
async fn create_or_update(
    req: HttpRequest,
    data: web::Data<AppState>,
    form: web::Form<ConfigForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Config)
            .api_type(ApiType::ConsoleApi)
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
    let namespace_transferred = config_form.namespace_id.is_empty();

    if namespace_transferred {
        config_form.namespace_id = model::common::DEFAULT_NAMESPACE_ID.to_string();
    }

    config_form.namespace_id = config_form.namespace_id.trim().to_string();
    config_form.r#type = ConfigType::from_str(&config_form.r#type)
        .unwrap_or(ConfigType::default())
        .to_string();

    let auth_content = req.extensions().get::<AuthContext>().unwrap().clone();
    let src_user = config_form
        .src_user
        .clone()
        .unwrap_or(auth_content.username);

    let src_ip = String::from(
        req.connection_info()
            .realip_remote_addr()
            .unwrap_or_default(),
    );

    let _ = service::config::create_or_update(
        &data.database_connection,
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

#[delete("")]
async fn delete(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<DeleteParam>,
    form: Option<web::Form<DeleteForm>>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Config)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let mut tenant = params.tenant.to_string();

    if let Some(delete_form) = form
        && let namespace_id = delete_form.namespace_id.to_string()
        && !namespace_id.is_empty()
        && tenant.is_empty()
    {
        tenant = namespace_id;
    }

    if tenant.is_empty() {
        tenant = DEFAULT_NAMESPACE_ID.to_string();
    }

    let client_ip = String::from(
        req.connection_info()
            .realip_remote_addr()
            .unwrap_or_default(),
    );

    let auth_content = req.extensions().get::<AuthContext>().unwrap().clone();

    let src_user = auth_content.username;

    service::config::delete(
        &data.database_connection,
        &params.data_id,
        &params.group_name,
        &tenant,
        "",
        &client_ip,
        &src_user,
        "",
    )
    .await
    .unwrap();

    model::common::Result::<bool>::http_success(true)
}

#[get("beta")]
async fn find_beta_one(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<ConfigForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let result = service::config::find_gray_one(
        &data.database_connection,
        &params.data_id,
        &params.group_name,
        &params.namespace_id,
    )
    .await
    .unwrap()
    .map(ConfigGrayInfo::from);

    model::common::Result::<Option<ConfigGrayInfo>>::http_success(result)
}

#[get("listener")]
async fn find_listeners(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<ConfigForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    model::common::Result::<Option<ConfigListenerInfo>>::http_success(ConfigListenerInfo {
        query_type: ConfigListenerInfo::QUERY_TYPE_CONFIG.to_string(),
        listeners_status: HashMap::new(),
    })
}

pub fn routes() -> Scope {
    web::scope("/cs/config")
        .service(find_one)
        .service(search)
        .service(create_or_update)
        .service(delete)
        .service(find_beta_one)
        .service(find_listeners)
}
