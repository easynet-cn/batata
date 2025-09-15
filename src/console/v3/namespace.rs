use std::sync::LazyLock;

use actix_web::{
    HttpMessage, HttpRequest, HttpResponse, Responder, Scope, delete, get, http::StatusCode, post,
    put, web,
};
use serde::Deserialize;

use crate::{
    ActionTypes, ApiType, Secured, SignType,
    auth::model::ONLY_IDENTITY,
    error::{self, BatataError},
    model::{
        common::{self, AppState},
        naming::Namespace,
    },
    secured, service,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetParam {
    namespace_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateFormData {
    custom_namespace_id: Option<String>,
    namespace_id: Option<String>,
    namespace_name: String,
    namespace_desc: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateFormData {
    namespace_id: String,
    namespace_name: String,
    namespace_desc: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckParam {
    custom_namespace_id: String,
}

const NAMESPACE_ID_MAX_LENGTH: usize = 128;

#[get("")]
async fn get(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<GetParam>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/namespaces")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    match service::namespace::get_by_namespace_id(
        &data.database_connection,
        &params.namespace_id,
        "1",
    )
    .await
    {
        Ok(namespace) => common::Result::<Namespace>::http_success(namespace),
        Err(err) => {
            if let Some(BatataError::ApiError(status, code, message, data)) = err.downcast_ref() {
                common::Result::<String>::http_response(
                    *status as u16,
                    *code,
                    message.to_string(),
                    data.to_string(),
                )
            } else {
                HttpResponse::InternalServerError().body(err.to_string())
            }
        }
    }
}

#[get("list")]
async fn find_all(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/namespaces")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .tags(vec![ONLY_IDENTITY.to_string()])
            .build()
    );

    let namespaces: Vec<Namespace> = service::namespace::find_all(&data.database_connection).await;

    common::Result::<Vec<Namespace>>::http_success(namespaces)
}

#[post("")]
async fn create(
    req: HttpRequest,
    data: web::Data<AppState>,
    form: web::Form<CreateFormData>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/namespaces")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let mut namespace_id = form
        .custom_namespace_id
        .clone()
        .unwrap_or_default()
        .trim()
        .to_string();
    let namespace_name = form.namespace_name.trim().to_string();
    let namespace_desc = form.namespace_desc.clone().unwrap_or_default();

    if namespace_id.is_empty() {
        namespace_id = uuid::Uuid::new_v4().to_string();
    }

    let regex = regex::Regex::new(r"^[\w-]+").unwrap();

    if !regex.is_match(&namespace_id) {
        return common::Result::<String>::http_response(
            StatusCode::NOT_FOUND.as_u16(),
            error::ILLEGAL_NAMESPACE.code,
            error::ILLEGAL_NAMESPACE.message.to_string(),
            format!("namespaceId [{}] mismatch the pattern", namespace_id),
        );
    }

    if namespace_id.chars().count() > NAMESPACE_ID_MAX_LENGTH {
        return common::Result::<String>::http_response(
            StatusCode::NOT_FOUND.as_u16(),
            error::ILLEGAL_NAMESPACE.code,
            error::ILLEGAL_NAMESPACE.message.to_string(),
            format!("too long namespaceId, over {}", namespace_id),
        );
    }

    if !namespace_name_check(&namespace_name) {
        return common::Result::<String>::http_response(
            StatusCode::NOT_FOUND.as_u16(),
            error::ILLEGAL_NAMESPACE.code,
            error::ILLEGAL_NAMESPACE.message.to_string(),
            format!("namespaceName [{}] contains illegal char", namespace_name),
        );
    }

    let res = service::namespace::create(
        &data.database_connection,
        &namespace_id,
        &namespace_name,
        &namespace_desc,
    )
    .await;

    common::Result::<bool>::http_success(res.is_ok())
}

#[put("")]
async fn update(
    req: HttpRequest,
    data: web::Data<AppState>,
    form: web::Form<UpdateFormData>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/namespaces")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    if form.namespace_id.is_empty() {
        return common::Result::<String>::http_response(
            StatusCode::BAD_REQUEST.as_u16(),
            error::PARAMETER_MISSING.code,
            error::PARAMETER_MISSING.message.to_string(),
            "required parameter 'namespaceId' is missing".to_string(),
        );
    }

    if form.namespace_name.is_empty() {
        return common::Result::<String>::http_response(
            StatusCode::BAD_REQUEST.as_u16(),
            error::PARAMETER_MISSING.code,
            error::PARAMETER_MISSING.message.to_string(),
            "required parameter 'namespaceName' is missing".to_string(),
        );
    }

    if !namespace_name_check(&form.namespace_name) {
        return common::Result::<String>::http_response(
            StatusCode::NOT_FOUND.as_u16(),
            error::ILLEGAL_NAMESPACE.code,
            error::ILLEGAL_NAMESPACE.message.to_string(),
            format!(
                "namespaceName [{}] contains illegal char",
                &form.namespace_name
            ),
        );
    }

    let namespace_desc = form.namespace_desc.clone().unwrap_or_default();

    let res = service::namespace::update(
        &data.database_connection,
        &form.namespace_id,
        &form.namespace_name,
        &namespace_desc,
    )
    .await
    .unwrap();

    common::Result::<bool>::http_success(res)
}

#[delete("")]
async fn delete(
    req: HttpRequest,
    data: web::Data<AppState>,
    form: web::Query<GetParam>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/namespaces")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let res = service::namespace::delete(&data.database_connection, &form.namespace_id).await;

    common::Result::<bool>::http_success(res.unwrap_or_default())
}

#[get("exist")]
async fn exist(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<CheckParam>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/namespaces")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .tags(vec![ONLY_IDENTITY.to_string()])
            .build()
    );

    if params.custom_namespace_id.is_empty() {
        return common::Result::<bool>::http_success(false);
    }

    let result =
        service::namespace::check(&data.database_connection, &params.custom_namespace_id).await;

    match result {
        Ok(e) => common::Result::<bool>::http_success(e),
        Err(err) => {
            if let Some(BatataError::ApiError(status, code, message, data)) = err.downcast_ref() {
                common::Result::<String>::http_response(
                    *status as u16,
                    *code,
                    message.to_string(),
                    data.to_string(),
                )
            } else {
                HttpResponse::InternalServerError().body(err.to_string())
            }
        }
    }
}

fn namespace_name_check(namespace_name: &str) -> bool {
    static RE: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"^[^@#$%^&*]+$").unwrap());

    RE.is_match(namespace_name)
}

pub fn routes() -> Scope {
    web::scope("/core/namespace")
        .service(get)
        .service(find_all)
        .service(create)
        .service(update)
        .service(delete)
        .service(exist)
}
