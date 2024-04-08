use actix_web::{get, web, HttpResponse, Responder, Scope};
use serde::Deserialize;

use crate::api::model::AppState;
use crate::common::model::RestResult;
use crate::core::model::Namespace;
use crate::service;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetNamespaceParams {
    show: Option<String>,
    namespace_id: Option<String>,
}

#[get("")]
pub async fn get_namespaces(
    data: web::Data<AppState>,
    params: web::Query<GetNamespaceParams>,
) -> impl Responder {
    if params.show.is_some() && params.show.as_ref().unwrap() == "all" {
        let namespace = service::namespace::get_by_namespace_id(
            data.conns.get(0).unwrap(),
            params.namespace_id.as_ref().unwrap().to_string(),
        )
        .await;

        return HttpResponse::Ok().json(namespace);
    }

    let namespaces: Vec<Namespace> = service::namespace::find_all(data.conns.get(0).unwrap()).await;
    let rest_result = RestResult::<Vec<Namespace>>::success(namespaces);

    return HttpResponse::Ok().json(rest_result);
}

pub fn routers() -> Scope {
    web::scope("/namespaces").service(get_namespaces)
}
