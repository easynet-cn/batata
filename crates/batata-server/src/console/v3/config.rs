use std::str::FromStr;

use actix_multipart::Multipart;
use actix_web::{
    HttpMessage, HttpRequest, HttpResponse, Responder, Scope, delete, get, http::StatusCode, post,
    web,
};
use futures::StreamExt;
use serde::Deserialize;

use chrono::Utc;

use crate::{
    ActionTypes, ApiType, Secured, SignType,
    api::{
        config::model::{ConfigBasicInfo, ConfigDetailInfo, ConfigGrayInfo, ConfigListenerInfo},
        model::Page,
    },
    auth::model::AuthContext,
    config::{
        export_model::{ExportRequest, ImportRequest, ImportResult},
        model::{ConfigAllInfo, ConfigForm, ConfigType},
    },
    error, is_valid,
    model::{
        self,
        common::{AppState, DEFAULT_NAMESPACE_ID, ErrorResult},
    },
    secured,
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
    #[serde(default)]
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

    let namespace_id = if params.namespace_id.is_empty() {
        model::common::DEFAULT_NAMESPACE_ID.to_string()
    } else {
        params.namespace_id.clone()
    };

    let result = match data
        .console_datasource
        .config_find_one(&params.data_id, &params.group_name, &namespace_id)
        .await
    {
        Ok(config) => config.map(ConfigDetailInfo::from),
        Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
    };

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
        .filter(|e| !e.is_empty())
        .map(|e| e.to_string())
        .collect::<Vec<String>>();
    let types = search_param
        .config_form
        .r#type
        .split(",")
        .filter(|e| !e.is_empty())
        .map(|e| e.to_string())
        .collect::<Vec<String>>();

    let search_ns = if search_param.config_form.namespace_id.is_empty() {
        model::common::DEFAULT_NAMESPACE_ID.to_string()
    } else {
        search_param.config_form.namespace_id.clone()
    };

    let result = data
        .console_datasource
        .config_search_page(
            search_param.page_no,
            search_param.page_size,
            &search_ns,
            &search_param.config_form.data_id,
            &search_param.config_form.group_name,
            &search_param.config_form.app_name,
            tags,
            types,
            &search_param.config_form.content,
        )
        .await;

    match result {
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
    }
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
        .unwrap_or_default()
        .to_string();

    let auth_content = match req.extensions().get::<AuthContext>() {
        Some(ctx) => ctx.clone(),
        None => return HttpResponse::Unauthorized().body("Unauthorized"),
    };
    let src_user = config_form.src_user.take().unwrap_or(auth_content.username);

    let src_ip = req
        .connection_info()
        .realip_remote_addr()
        .unwrap_or_default()
        .to_owned();

    let _ = data
        .console_datasource
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

#[delete("")]
async fn delete(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<DeleteParam>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Config)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let tenant = if params.namespace_id.is_empty() {
        DEFAULT_NAMESPACE_ID.to_string()
    } else {
        params.namespace_id.to_string()
    };

    let client_ip = req
        .connection_info()
        .realip_remote_addr()
        .unwrap_or_default()
        .to_owned();

    let auth_content = match req.extensions().get::<AuthContext>() {
        Some(ctx) => ctx.clone(),
        None => return HttpResponse::Unauthorized().body("Unauthorized"),
    };

    let src_user = auth_content.username;

    if let Err(e) = data
        .console_datasource
        .config_delete(
            &params.data_id,
            &params.group_name,
            &tenant,
            "",
            &client_ip,
            &src_user,
            "",
        )
        .await
    {
        return HttpResponse::InternalServerError().body(e.to_string());
    }

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

    let beta_ns = if params.namespace_id.is_empty() {
        model::common::DEFAULT_NAMESPACE_ID.to_string()
    } else {
        params.namespace_id.clone()
    };

    let result = match data
        .console_datasource
        .config_find_gray_one(&params.data_id, &params.group_name, &beta_ns)
        .await
    {
        Ok(config) => config,
        Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
    };

    model::common::Result::<Option<ConfigGrayInfo>>::http_success(result)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct BetaPublishForm {
    pub data_id: String,
    #[serde(default)]
    pub group_name: String,
    #[serde(default)]
    pub namespace_id: String,
    pub content: String,
    #[serde(default)]
    pub beta_ips: String,
    #[serde(default)]
    pub app_name: String,
    #[serde(default)]
    pub r#type: String,
    #[serde(default)]
    pub encrypted_data_key: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BetaQueryParam {
    pub data_id: String,
    #[serde(default)]
    pub group_name: String,
    #[serde(default)]
    pub namespace_id: String,
    #[serde(default)]
    pub tenant: String,
}

#[post("beta")]
async fn publish_beta(
    req: HttpRequest,
    data: web::Data<AppState>,
    form: web::Form<BetaPublishForm>,
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

    if form.content.is_empty() {
        return model::common::Result::<String>::http_response(
            StatusCode::BAD_REQUEST.as_u16(),
            error::PARAMETER_MISSING.code,
            error::PARAMETER_MISSING.message.to_string(),
            "Required parameter 'content' type String is not present",
        );
    }

    if form.beta_ips.is_empty() {
        return model::common::Result::<String>::http_response(
            StatusCode::BAD_REQUEST.as_u16(),
            error::PARAMETER_MISSING.code,
            error::PARAMETER_MISSING.message.to_string(),
            "Required parameter 'betaIps' type String is not present",
        );
    }

    let mut namespace_id = form.namespace_id.clone();
    if namespace_id.is_empty() {
        namespace_id = DEFAULT_NAMESPACE_ID.to_string();
    }

    let group_name = if form.group_name.is_empty() {
        "DEFAULT_GROUP".to_string()
    } else {
        form.group_name.clone()
    };

    let auth_context = match req.extensions().get::<AuthContext>() {
        Some(ctx) => ctx.clone(),
        None => return HttpResponse::Unauthorized().body("Unauthorized"),
    };
    let src_user = auth_context.username;
    let src_ip = req
        .connection_info()
        .realip_remote_addr()
        .unwrap_or_default()
        .to_owned();

    // Build gray rule from betaIps
    let gray_rule_info = batata_config::model::gray_rule::GrayRulePersistInfo::new_beta(
        &form.beta_ips,
        batata_config::model::gray_rule::BetaGrayRule::PRIORITY,
    );
    let gray_rule = match gray_rule_info.to_json() {
        Ok(json) => json,
        Err(e) => {
            return model::common::Result::<String>::http_response(
                StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error::SERVER_ERROR.code,
                error::SERVER_ERROR.message.to_string(),
                format!("Failed to serialize gray rule: {}", e),
            );
        }
    };

    if let Err(e) = data
        .console_datasource
        .config_create_or_update_gray(
            &form.data_id,
            &group_name,
            &namespace_id,
            &form.content,
            "beta",
            &gray_rule,
            &src_user,
            &src_ip,
            &form.app_name,
            &form.encrypted_data_key,
        )
        .await
    {
        return HttpResponse::InternalServerError().body(e.to_string());
    }

    model::common::Result::<bool>::http_success(true)
}

#[delete("beta")]
async fn delete_beta(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<BetaQueryParam>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Config)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let mut namespace_id = params.namespace_id.clone();
    if namespace_id.is_empty() {
        namespace_id = params.tenant.clone();
    }
    if namespace_id.is_empty() {
        namespace_id = DEFAULT_NAMESPACE_ID.to_string();
    }

    let client_ip = req
        .connection_info()
        .realip_remote_addr()
        .unwrap_or_default()
        .to_owned();

    let src_user = match req.extensions().get::<AuthContext>() {
        Some(ctx) => ctx.username.clone(),
        None => return HttpResponse::Unauthorized().body("Unauthorized"),
    };

    if let Err(e) = data
        .console_datasource
        .config_delete_gray(
            &params.data_id,
            &params.group_name,
            &namespace_id,
            "beta",
            &client_ip,
            &src_user,
        )
        .await
    {
        return HttpResponse::InternalServerError().body(e.to_string());
    }

    model::common::Result::<bool>::http_success(true)
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

    match data
        .console_datasource
        .config_listener_list(&params.data_id, &params.group_name, &params.namespace_id)
        .await
    {
        Ok(listener_info) => {
            model::common::Result::<Option<ConfigListenerInfo>>::http_success(listener_info)
        }
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[get("export")]
async fn export_configs(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<ExportRequest>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let namespace_id = if params.namespace_id.is_empty() {
        DEFAULT_NAMESPACE_ID.to_string()
    } else {
        params.namespace_id.clone()
    };

    // Parse data_ids if provided
    let data_ids = params.data_ids.as_ref().map(|ids| {
        ids.split(',')
            .filter(|s| !s.is_empty())
            .map(|s| s.trim().to_string())
            .collect::<Vec<String>>()
    });

    // Export configs via datasource
    let zip_data = match data
        .console_datasource
        .config_export(
            &namespace_id,
            params.group.as_deref(),
            data_ids,
            params.app_name.as_deref(),
        )
        .await
    {
        Ok(z) => z,
        Err(e) => return HttpResponse::NotFound().body(e.to_string()),
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

#[post("import")]
async fn import_configs(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<ImportRequest>,
    mut payload: Multipart,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Config)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let namespace_id = if params.namespace_id.is_empty() {
        DEFAULT_NAMESPACE_ID.to_string()
    } else {
        params.namespace_id.clone()
    };

    let policy = params.get_policy();

    // Get user info
    let auth_context = match req.extensions().get::<AuthContext>() {
        Some(ctx) => ctx.clone(),
        None => return HttpResponse::Unauthorized().body("Unauthorized"),
    };

    let src_user = auth_context.username;
    let src_ip = req
        .connection_info()
        .realip_remote_addr()
        .unwrap_or_default()
        .to_owned();

    // Read file from multipart
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

    // Import configs via datasource
    let result = match data
        .console_datasource
        .config_import(file_data, &namespace_id, policy, &src_user, &src_ip)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return model::common::Result::<ImportResult>::http_response(
                StatusCode::BAD_REQUEST.as_u16(),
                error::PARAMETER_VALIDATE_ERROR.code,
                error::PARAMETER_VALIDATE_ERROR.message.to_string(),
                format!("Import failed: {}", e),
            );
        }
    };

    model::common::Result::<ImportResult>::http_success(result)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BatchDeleteParam {
    pub ids: String,
}

#[delete("batchDelete")]
async fn batch_delete(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<BatchDeleteParam>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Config)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let ids: Vec<i64> = params
        .ids
        .split(',')
        .filter_map(|s| s.trim().parse::<i64>().ok())
        .collect();

    if ids.is_empty() {
        return model::common::Result::<String>::http_response(
            StatusCode::BAD_REQUEST.as_u16(),
            error::PARAMETER_MISSING.code,
            error::PARAMETER_MISSING.message.to_string(),
            "Required parameter 'ids' is missing or invalid",
        );
    }

    let client_ip = req
        .connection_info()
        .realip_remote_addr()
        .unwrap_or_default()
        .to_owned();

    let src_user = match req.extensions().get::<AuthContext>() {
        Some(ctx) => ctx.username.clone(),
        None => return HttpResponse::Unauthorized().body("Unauthorized"),
    };

    match data
        .console_datasource
        .config_batch_delete(&ids, &client_ip, &src_user)
        .await
    {
        Ok(_) => model::common::Result::<bool>::http_success(true),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct SearchDetailParam {
    #[serde(flatten)]
    config_form: ConfigForm,
    pub page_no: u64,
    pub page_size: u64,
    #[serde(default)]
    pub config_detail: String,
    #[serde(default = "default_search_type")]
    pub search: String,
}

fn default_search_type() -> String {
    "blur".to_string()
}

#[get("searchDetail")]
async fn search_detail(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<SearchDetailParam>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let tags = params
        .config_form
        .config_tags
        .split(',')
        .filter(|e| !e.is_empty())
        .map(|e| e.to_string())
        .collect::<Vec<String>>();
    let types = params
        .config_form
        .r#type
        .split(',')
        .filter(|e| !e.is_empty())
        .map(|e| e.to_string())
        .collect::<Vec<String>>();

    let content = if params.config_detail.is_empty() {
        &params.config_form.content
    } else {
        &params.config_detail
    };

    match data
        .console_datasource
        .config_search_page(
            params.page_no,
            params.page_size,
            &params.config_form.namespace_id,
            &params.config_form.data_id,
            &params.config_form.group_name,
            &params.config_form.app_name,
            tags,
            types,
            content,
        )
        .await
    {
        Ok(page_result) => {
            model::common::Result::<Page<ConfigBasicInfo>>::http_success(page_result)
        }
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListenerByIpParam {
    pub ip: String,
    #[serde(default)]
    pub all: bool,
    #[serde(default)]
    pub namespace_id: Option<String>,
}

#[get("listener/ip")]
async fn find_listener_by_ip(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<ListenerByIpParam>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let namespace_id = params.namespace_id.as_deref().unwrap_or("").to_string();

    match data
        .console_datasource
        .config_listener_list_by_ip(&params.ip, params.all, &namespace_id)
        .await
    {
        Ok(listener_info) => {
            model::common::Result::<ConfigListenerInfo>::http_success(listener_info)
        }
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConsoleCloneParam {
    pub target_namespace_id: String,
    #[serde(default = "default_clone_policy")]
    pub policy: String,
    #[serde(default)]
    pub src_user: Option<String>,
}

fn default_clone_policy() -> String {
    "ABORT".to_string()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct CloneConfigBean {
    pub cfg_id: i64,
    #[serde(default)]
    pub data_id: String,
    #[serde(default)]
    pub group: String,
}

#[post("clone")]
async fn clone_config(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<ConsoleCloneParam>,
    body: web::Json<Vec<CloneConfigBean>>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Config)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let clone_beans = body.into_inner();
    if clone_beans.is_empty() {
        return model::common::Result::<ImportResult>::http_response(
            StatusCode::BAD_REQUEST.as_u16(),
            error::NO_SELECTED_CONFIG.code,
            error::NO_SELECTED_CONFIG.message.to_string(),
            "No configuration selected",
        );
    }

    let mut namespace_id = params.target_namespace_id.trim().to_string();
    if namespace_id.is_empty() {
        namespace_id = DEFAULT_NAMESPACE_ID.to_string();
    }

    let src_user = params.src_user.clone().unwrap_or_else(|| {
        req.extensions()
            .get::<AuthContext>()
            .map(|ctx| ctx.username.clone())
            .unwrap_or_default()
    });
    let src_ip = req
        .connection_info()
        .realip_remote_addr()
        .unwrap_or_default()
        .to_owned();

    let ids: Vec<i64> = clone_beans.iter().map(|c| c.cfg_id).collect();

    match data
        .console_datasource
        .config_clone(&ids, &namespace_id, &params.policy, &src_user, &src_ip)
        .await
    {
        Ok(import_result) => model::common::Result::<ImportResult>::http_success(import_result),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

pub fn routes() -> Scope {
    web::scope("/cs/config")
        .service(find_one)
        .service(search)
        .service(search_detail)
        .service(create_or_update)
        .service(delete)
        .service(batch_delete)
        .service(find_beta_one)
        .service(publish_beta)
        .service(delete_beta)
        .service(find_listener_by_ip)
        .service(find_listeners)
        .service(export_configs)
        .service(import_configs)
        .service(clone_config)
}
