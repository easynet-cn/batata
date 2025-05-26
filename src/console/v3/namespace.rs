use actix_web::{HttpResponse, Responder, Scope, delete, get, post, put, web};
use serde::Deserialize;

use crate::{
    model::{
        common::{AppState, RestResult},
        naming::Namespace,
    },
    service,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetParam {
    show: Option<String>,
    namespace_id: Option<String>,
    check_namespace_id_exist: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateFormData {
    custom_namespace_id: Option<String>,
    namespace_name: String,
    namespace_desc: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateFormData {
    namespace: String,
    namespace_show_name: String,
    namespace_desc: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteParam {
    namespace_id: String,
}

const NAMESPACE_ID_MAX_LENGTH: usize = 128;

#[get("list")]
pub async fn get_all(data: web::Data<AppState>, params: web::Query<GetParam>) -> impl Responder {
    if params.show.is_some() && params.show.as_ref().unwrap() == "all" {
        let namespace = service::namespace::get_by_namespace_id(
            &data.database_connection,
            params.namespace_id.as_ref().unwrap().to_string(),
        )
        .await;

        return HttpResponse::Ok().json(namespace);
    }

    if params.check_namespace_id_exist.is_some() && params.check_namespace_id_exist.unwrap() {
        let count = service::namespace::get_count_by_tenant_id(
            &data.database_connection,
            params.namespace_id.as_ref().unwrap().to_string(),
        )
        .await;

        return HttpResponse::Ok().json(count > 0);
    }

    let namespaces: Vec<Namespace> = service::namespace::find_all(&data.database_connection).await;
    let rest_result = RestResult::<Vec<Namespace>>::success(namespaces);

    return HttpResponse::Ok().json(rest_result);
}

#[post("")]
pub async fn create(data: web::Data<AppState>, form: web::Form<CreateFormData>) -> impl Responder {
    let namespace_id: String;

    if form.custom_namespace_id.is_some() && !form.custom_namespace_id.as_ref().unwrap().is_empty()
    {
        namespace_id = form
            .custom_namespace_id
            .as_ref()
            .unwrap()
            .trim()
            .to_string();

        let regex = regex::Regex::new(r"^[\w-]+").unwrap();

        if !regex.is_match(&namespace_id) {
            return HttpResponse::Ok().json(false);
        }

        if namespace_id.len() > NAMESPACE_ID_MAX_LENGTH {
            return HttpResponse::Ok().json(false);
        }

        if service::namespace::get_count_by_tenant_id(
            &data.database_connection,
            namespace_id.clone(),
        )
        .await
            > 0
        {
            return HttpResponse::Ok().json(false);
        }
    } else {
        namespace_id = uuid::Uuid::new_v4().to_string();
    }

    let regex = regex::Regex::new(r"^[^@#$%^&*]+$").unwrap();

    if !regex.is_match(&form.namespace_name) {
        return HttpResponse::Ok().json(false);
    }

    let namespace_desc: String;

    if form.namespace_desc.is_some() {
        namespace_desc = form.namespace_desc.as_ref().unwrap().to_string();
    } else {
        namespace_desc = "".to_string();
    }

    let res = service::namespace::create(
        &data.database_connection,
        namespace_id,
        form.namespace_name.clone(),
        namespace_desc,
    )
    .await;

    return HttpResponse::Ok().json(res);
}

#[put("")]
pub async fn update(data: web::Data<AppState>, form: web::Form<UpdateFormData>) -> impl Responder {
    let regex = regex::Regex::new(r"^[^@#$%^&*]+$").unwrap();

    if !regex.is_match(&form.namespace_show_name) {
        return HttpResponse::Ok().json(false);
    }

    let namespace_desc: String;

    if form.namespace_desc.is_some() {
        namespace_desc = form.namespace_desc.as_ref().unwrap().to_string();
    } else {
        namespace_desc = "".to_string();
    }

    let res = service::namespace::update(
        &data.database_connection,
        form.namespace.clone(),
        form.namespace_show_name.clone(),
        namespace_desc,
    )
    .await;

    return HttpResponse::Ok().json(res);
}

#[delete("")]
pub async fn delete(data: web::Data<AppState>, form: web::Query<DeleteParam>) -> impl Responder {
    let res =
        service::namespace::delete(&data.database_connection, form.namespace_id.clone()).await;

    return HttpResponse::Ok().json(res);
}

pub fn routers() -> Scope {
    web::scope("/core/namespace")
        .service(get_all)
        .service(create)
        .service(update)
        .service(delete)
}
